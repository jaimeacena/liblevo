import re
import subprocess
import sys
from pathlib import Path, PurePosixPath

import pytest

import parsezen.markdown_export as export_module
import parsezen.output as output_module
from parsezen.document_model import ConvertedResource
from parsezen.domain.jobs import MarkdownOrganization
from parsezen.errors import FinalIntegrityError, OutputWriteError
from parsezen.final_integrity import text_integrity_capture
from parsezen.markdown_export import prepare_markdown_export
from parsezen.output import replace_markdown_output, write_conversion_output


def test_chapter_export_builds_an_index_and_portable_relative_images(tmp_path: Path) -> None:
    source = tmp_path / "book.pdf"
    source.write_bytes(b"pdf")
    markdown = """# A useful book

<!-- PZDOC PDF PAGE 1 -->

## First chapter

![Figure](<__parsezen_resources__/pdf/figure.png>)

<!-- PZDOC PDF PAGE 8 -->

## Second chapter

Final text.
"""
    resource = ConvertedResource(PurePosixPath("pdf/figure.png"), b"image", "image/png")

    result = write_conversion_output(
        source,
        markdown,
        resources=(resource,),
        markdown_organization=MarkdownOrganization.BY_CHAPTER,
        markdown_include_metadata=True,
        markdown_include_page_references=True,
    )

    assert result.name == "book.md"
    index = result.read_text(encoding="utf-8")
    assert 'source: "book.pdf"' in index
    assert "# A useful book" in index
    assert "[First chapter](<book.chapters/01-first-chapter.md>)" in index
    first = (tmp_path / "book.chapters" / "01-first-chapter.md").read_text(encoding="utf-8")
    assert "> Página original 1" in first
    assert "../book.assets/pdf/figure.png" in first
    assert (tmp_path / "book.assets" / "pdf" / "figure.png").read_bytes() == b"image"


def test_reviewed_chapter_export_replaces_companion_and_can_return_to_one_file(
    tmp_path: Path,
) -> None:
    destination = tmp_path / "book.md"
    destination.write_text("- [Old](<book.chapters/01-old.md>)\n", encoding="utf-8")
    (tmp_path / "book.chapters").mkdir()
    (tmp_path / "book.chapters" / "01-old.md").write_text("old", encoding="utf-8")

    replace_markdown_output(
        destination,
        "# Book\n\n## One\n\nFirst.\n\n## Two\n\nSecond.",
        organization=MarkdownOrganization.BY_CHAPTER,
        source_name="source.pdf",
        include_metadata=False,
        include_page_references=False,
    )
    assert not (tmp_path / "book.chapters").exists()
    published_chapters = _chapter_paths(destination)
    assert len(published_chapters) == 2
    assert all(path.exists() for path in published_chapters)

    replace_markdown_output(
        destination,
        "# Book\n\nOne complete document.",
        organization=MarkdownOrganization.BY_CHAPTER,
        source_name="source.pdf",
        include_metadata=False,
        include_page_references=False,
    )
    assert not (tmp_path / "book.chapters").exists()
    assert not published_chapters[0].parent.exists()
    assert "One complete document" in destination.read_text(encoding="utf-8")


def test_single_file_export_keeps_private_markers_for_a_pending_review() -> None:
    exported = prepare_markdown_export(
        "<!-- PZDOC PDF PAGE 4 -->\n\nText",
        organization=MarkdownOrganization.SINGLE_FILE,
        source_name="source.pdf",
        include_metadata=False,
        include_page_references=False,
    )

    assert "PZDOC" in exported.primary_markdown
    assert exported.primary_markdown.rstrip().endswith("Text")


def _chapter_paths(destination: Path) -> list[Path]:
    return [
        destination.parent / target
        for target in re.findall(r"(?m)^- \[.*\]\(<([^>]+\.md)>\)$", destination.read_text("utf-8"))
    ]


def _publish(directory: Path, markdown: str, route: str) -> Path:
    capture = text_integrity_capture(markdown, markdown=True)
    options = dict(
        validate_staged=capture,
        markdown_organization=MarkdownOrganization.BY_CHAPTER,
        markdown_include_metadata=True,
        markdown_include_page_references=True,
    )
    if route == "conversion":
        result = write_conversion_output(directory / "book.pdf", markdown, **options)
    elif route == "improvement":
        result, _ = output_module.write_improvement_outputs(
            directory / "book.pdf", markdown, markdown, keep_raw=True, **options
        )
    else:
        result = directory / "book.md"
        replace_markdown_output(
            result,
            markdown,
            organization=MarkdownOrganization.BY_CHAPTER,
            source_name="book.pdf",
            include_metadata=True,
            include_page_references=True,
            validate_staged=capture,
        )
    assert capture.report is not None and capture.report.verified
    return result


@pytest.mark.parametrize("route", ["conversion", "improvement", "review"])
def test_all_publication_routes_preserve_preamble_and_validate_actual_chapters(
    tmp_path: Path, route: str
) -> None:
    markdown = (
        "# Book\n\nAdvertencia: conservar esta frase.\n\n"
        "<!-- PZDOC PDF PAGE 1 -->\n\n## One\n\nFirst.\n\n## Two\n\nSecond."
    )
    result = _publish(tmp_path, markdown, route)
    index = result.read_text("utf-8")
    chapters = _chapter_paths(result)
    assert "Advertencia: conservar esta frase." in index
    assert len(chapters) == 2
    assert "> Página original 1" in chapters[0].read_text("utf-8")
    assert "First." in chapters[0].read_text("utf-8")
    assert "Second." in chapters[1].read_text("utf-8")


@pytest.mark.parametrize("route", ["conversion", "improvement", "review"])
@pytest.mark.parametrize("damage", ["index", "chapter", "missing_chapter"])
def test_publication_rejects_damaged_actual_files(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch, route: str, damage: str
) -> None:
    stage_chapters = output_module._stage_chapter_directory
    # Conversion uses _write_exclusively for the index, so corrupt its encoded bytes at the
    # same filesystem boundary as the other routes, immediately before the integrity read.
    read_bytes = Path.read_bytes

    def damaged_read(path: Path) -> bytes:
        data = read_bytes(path)
        if damage == "index" and b"## \xc3\x8dndice" in data:
            return data.replace(b"## \xc3\x8dndice", b"missing index")
        return data

    def damaged_chapters(*args, **kwargs):
        directory = stage_chapters(*args, **kwargs)
        chapter = next(directory.glob("*.md"))
        if damage == "chapter":
            chapter.write_text("lost content", encoding="utf-8")
        elif damage == "missing_chapter":
            chapter.unlink()
        return directory

    monkeypatch.setattr(Path, "read_bytes", damaged_read)
    monkeypatch.setattr(output_module, "_stage_chapter_directory", damaged_chapters)
    previous = tmp_path / "book.md"
    if route == "review":
        previous.write_text("previous result", encoding="utf-8")
    with pytest.raises(FinalIntegrityError):
        _publish(tmp_path, "# Book\n\n## One\n\nFirst.\n\n## Two\n\nSecond.", route)
    if route == "review":
        assert previous.read_text("utf-8") == "previous result"
    else:
        assert not tuple(tmp_path.glob("*.md"))
    assert not tuple(tmp_path.glob("*chapters*"))
    assert not tuple(tmp_path.glob(".parsezen-*.tmp"))


def test_splitter_conservation_guard_rejects_an_omitted_preamble(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    split = export_module._split_chapters
    monkeypatch.setattr(export_module, "_split_chapters", lambda text: ("", split(text)[1]))
    with pytest.raises(FinalIntegrityError, match="no conserva"):
        prepare_markdown_export(
            "Advertencia: importante.\n\n## One\n\nFirst.\n\n## Two\n\nSecond.",
            organization=MarkdownOrganization.BY_CHAPTER,
            source_name="book.md",
            include_metadata=False,
            include_page_references=False,
        )


@pytest.mark.parametrize("with_resources", [False, True])
def test_relative_links_are_relocated_even_alongside_extracted_images(
    tmp_path: Path, with_resources: bool
) -> None:
    resource = ConvertedResource(PurePosixPath("figure.png"), b"image", "image/png")
    markdown = (
        '# Book\n\n## One\n\n[Local](./appendix.pdf "Title")\n\n'
        '[Parent](../shared.pdf)\n\n[Complex](<dir/a(b).pdf> "Other")\n\n'
        "![Extracted](<__parsezen_resources__/figure.png>)\n\n## Two\n\nSecond."
    )
    result = write_conversion_output(
        tmp_path / "book.pdf",
        markdown,
        resources=(resource,) if with_resources else (),
        markdown_organization=MarkdownOrganization.BY_CHAPTER,
    )
    first = _chapter_paths(result)[0].read_text("utf-8")
    assert '[Local](.././appendix.pdf "Title")' in first
    assert "[Parent](../../shared.pdf)" in first
    assert '[Complex](<../dir/a(b).pdf> "Other")' in first
    if with_resources:
        assert "../book.assets/figure.png" in first
        assert "../../book.assets" not in first


@pytest.mark.parametrize(
    "navigation",
    [
        "[Next](#two)",
        "[Next][ref]\n\n[ref]: appendix.pdf",
        '<a href="appendix.pdf">Next</a>',
        "`[Example](./unchanged.pdf)`",
        "![Embedded](#figure)",
    ],
)
def test_document_scope_and_code_are_preserved_when_safe_relocation_is_not_proven(
    tmp_path: Path, navigation: str
) -> None:
    markdown = f"# Book\n\n## One\n\n{navigation}\n\n## Two\n\nSecond."
    result = write_conversion_output(
        tmp_path / "book.pdf", markdown, markdown_organization=MarkdownOrganization.BY_CHAPTER
    )
    assert result.read_text("utf-8") == markdown
    assert not _chapter_paths(result)


def test_long_fences_and_nested_headings_do_not_create_chapters(tmp_path: Path) -> None:
    markdown = (
        "# Book\n\n## One\n\n````markdown\n```\n## Example\n````\n\n"
        "> ## Quoted\n\n## Two\n\nSecond."
    )
    result = write_conversion_output(
        tmp_path / "book.pdf", markdown, markdown_organization=MarkdownOrganization.BY_CHAPTER
    )
    assert len(_chapter_paths(result)) == 2
    assert "## Example" in _chapter_paths(result)[0].read_text("utf-8")


def test_preamble_code_indentation_is_preserved_in_the_index(tmp_path: Path) -> None:
    result = write_conversion_output(
        tmp_path / "book.pdf",
        "    example: keep indentation\n\n## One\n\nFirst.\n\n## Two\n\nSecond.",
        markdown_organization=MarkdownOrganization.BY_CHAPTER,
        markdown_include_metadata=True,
    )
    assert "\n    example: keep indentation\n" in result.read_text("utf-8")
    assert len(_chapter_paths(result)) == 2


def test_existing_front_matter_keeps_its_single_document_scope(tmp_path: Path) -> None:
    markdown = "---\ntitle: Book\n---\n\n## One\n\nFirst.\n\n## Two\n\nSecond."
    result = write_conversion_output(
        tmp_path / "book.pdf", markdown, markdown_organization=MarkdownOrganization.BY_CHAPTER
    )
    assert result.read_text("utf-8") == markdown
    assert not _chapter_paths(result)


def test_malformed_bracket_runs_do_not_discard_content(tmp_path: Path) -> None:
    malformed = "[" * 20_000
    result = write_conversion_output(
        tmp_path / "book.pdf",
        f"## One\n\n{malformed}\n\n## Two\n\nSecond.",
        markdown_organization=MarkdownOrganization.BY_CHAPTER,
    )
    assert malformed in _chapter_paths(result)[0].read_text("utf-8")


@pytest.mark.parametrize("after_commit", [False, True])
def test_process_interruption_keeps_a_complete_linked_generation(
    tmp_path: Path, after_commit: bool
) -> None:
    result = write_conversion_output(
        tmp_path / "book.pdf",
        "# Book\n\n## Old one\n\nPrevious first.\n\n## Old two\n\nPrevious second.",
        markdown_organization=MarkdownOrganization.BY_CHAPTER,
    )
    previous = result.read_bytes()
    script = """
import os
import sys
from pathlib import Path
import parsezen.output as output
from parsezen.domain.jobs import MarkdownOrganization
replace = output.os.replace
def interrupted(source, destination):
    if sys.argv[2] == "True":
        replace(source, destination)
    os._exit(47)
output.os.replace = interrupted
output.replace_markdown_output(
    Path(sys.argv[1]), "# Book\\n\\n## New one\\n\\nNext first.\\n\\n## New two\\n\\nNext second.",
    organization=MarkdownOrganization.BY_CHAPTER, source_name="book.pdf",
    include_metadata=False, include_page_references=False,
)
"""
    child = subprocess.run(
        [sys.executable, "-B", "-c", script, str(result), str(after_commit)],
        capture_output=True,
        timeout=30,
        check=False,
    )
    assert child.returncode == 47, child.stderr.decode(errors="replace")
    assert (result.read_bytes() == previous) is not after_commit
    chapters = _chapter_paths(result)
    assert len(chapters) == 2
    assert all(path.exists() for path in chapters)
    assert ("Next first." if after_commit else "Previous first.") in chapters[0].read_text("utf-8")


def test_failed_replace_preserves_old_chapters_and_success_leaves_unowned_files(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    result = write_conversion_output(
        tmp_path / "book.pdf",
        "## One\n\nFirst.\n\n## Two\n\nSecond.",
        markdown_organization=MarkdownOrganization.BY_CHAPTER,
    )
    chapters = _chapter_paths(result)
    extra = chapters[0].parent / "personal-note.txt"
    extra.write_text("keep", encoding="utf-8")
    previous = result.read_bytes()
    with monkeypatch.context() as patch:

        def fail(*_args):
            raise OSError("locked")

        patch.setattr(output_module.os, "replace", fail)
        with pytest.raises(OutputWriteError):
            _publish(tmp_path, "## New one\n\nNew first.\n\n## New two\n\nNew second.", "review")
    assert result.read_bytes() == previous
    assert all(path.exists() for path in chapters)
    _publish(tmp_path, "## New one\n\nNew first.\n\n## New two\n\nNew second.", "review")
    assert all(not path.exists() for path in chapters)
    assert extra.read_text("utf-8") == "keep"
