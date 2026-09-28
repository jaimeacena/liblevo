"""Derive portable Markdown layouts from one canonical transformed document."""

from __future__ import annotations

import re
import unicodedata
from collections.abc import Iterator
from dataclasses import dataclass
from pathlib import Path

from markdown_it import MarkdownIt
from markdown_it.helpers import parseLinkDestination
from markdown_it.token import Token

from liblevo.document_model import RESOURCE_REFERENCE_PREFIX
from liblevo.domain.jobs import MarkdownOrganization
from liblevo.errors import FinalIntegrityError

_HEADING = re.compile(r"^(#{1,6})[ \t]+(.+?)\s*$")
_PAGE_MARKER = re.compile(r"<!--\s*PZDOC PDF PAGE (\d+)\s*-->", re.IGNORECASE)
_UNSAFE_FILENAME = re.compile(r"[^a-z0-9]+")
_MARKDOWN_LINK_START = re.compile(r"!?\[(?:\\.|[^\[\]\\])*\]\(\s*")


@dataclass(frozen=True, slots=True)
class MarkdownChapter:
    filename: str
    title: str
    markdown: str


@dataclass(frozen=True, slots=True)
class MarkdownExport:
    primary_markdown: str
    chapters: tuple[MarkdownChapter, ...] = ()


def prepare_markdown_export(
    markdown: str,
    *,
    organization: MarkdownOrganization,
    source_name: str,
    include_metadata: bool,
    include_page_references: bool,
    chapter_directory: str = "",
) -> MarkdownExport:
    """Create a publication variant without mutating the canonical transformed text."""

    canonical = markdown
    prepared = _page_references(markdown, visible=include_page_references)
    metadata = _metadata_block(prepared, source_name) if include_metadata else ""
    if organization is not MarkdownOrganization.BY_CHAPTER:
        return MarkdownExport(_join(metadata, prepared) if metadata else prepared)

    preamble, source_chapters = _split_chapters(canonical)
    if source_chapters and preamble + "".join(c.markdown for c in source_chapters) != canonical:
        raise FinalIntegrityError("La división Markdown no conserva todo el contenido aprobado.")
    chapters_list: list[MarkdownChapter] = []
    for chapter in source_chapters:
        relocated = rebase_relative_markdown_links(chapter.markdown)
        if relocated is None:
            # Keep the portable single document when relocation cannot be demonstrated.
            return MarkdownExport(_join(metadata, prepared) if metadata else prepared)
        chapters_list.append(
            MarkdownChapter(
                chapter.filename,
                chapter.title,
                _page_references(relocated, visible=include_page_references).strip("\r\n") + "\n",
            )
        )
    chapters = tuple(chapters_list)
    if len(chapters) < 2:
        return MarkdownExport(_join(metadata, prepared) if metadata else prepared)

    document_title = _document_title(prepared) or Path(source_name).stem
    introduction = _page_references(preamble, visible=include_page_references).strip("\r\n")
    if not _document_title(introduction):
        introduction = _join(f"# {document_title}", introduction).strip("\r\n")
    index_content = [introduction, "", "## Índice", ""]
    index_lines = [metadata.rstrip(), "", *index_content] if metadata else index_content
    for chapter in chapters:
        target = (
            f"{chapter_directory}/{chapter.filename}" if chapter_directory else chapter.filename
        )
        index_lines.append(f"- [{_markdown_link_text(chapter.title)}](<{target}>)")
    return MarkdownExport("\n".join(index_lines).strip("\r\n") + "\n", chapters)


def rebase_relative_markdown_links(markdown: str) -> str | None:
    """Relocate inline destinations only if the parser confirms no other rendered change."""

    parser = _markdown_parser()
    tokens = parser.parse(markdown)
    insertions: list[int] = []
    for match in _MARKDOWN_LINK_START.finditer(markdown):
        start = match.end()
        destination = parseLinkDestination(markdown, start, len(markdown))
        if destination.ok and _rebased_target(destination.str) != destination.str:
            insertions.append(start + (1 if markdown[start : start + 1] == "<" else 0))
    pieces: list[str] = []
    previous = 0
    for position in insertions:
        pieces.extend((markdown[previous:position], "../"))
        previous = position
    pieces.append(markdown[previous:])
    relocated = "".join(pieces)
    for token in _walk_tokens(tokens):
        attribute = (
            "href" if token.type == "link_open" else "src" if token.type == "image" else None
        )
        if attribute is not None:
            target = token.attrGet(attribute)
            if isinstance(target, str):
                token.attrSet(attribute, parser.normalizeLink(_rebased_target(target)))
    expected = parser.renderer.render(tokens, parser.options, {})
    return relocated if parser.render(relocated) == expected else None


def _rebased_target(target: str) -> str:
    if (
        not target
        or target.startswith(("#", "/", "\\", RESOURCE_REFERENCE_PREFIX))
        or re.match(r"^[a-z][a-z0-9+.-]*:", target, re.IGNORECASE)
    ):
        return target
    return f"../{target}"


def _markdown_parser() -> MarkdownIt:
    return MarkdownIt("commonmark").enable(["table", "strikethrough"])


def _walk_tokens(tokens: list[Token]) -> Iterator[Token]:
    for token in tokens:
        yield token
        if token.children:
            yield from _walk_tokens(token.children)


def _page_references(markdown: str, *, visible: bool) -> str:
    if not visible:
        return markdown

    def replace(match: re.Match[str]) -> str:
        number = match.group(1)
        return f'<a id="pagina-{number}"></a>\n\n> Página original {number}'

    return _PAGE_MARKER.sub(replace, markdown)


def _metadata_block(markdown: str, source_name: str) -> str:
    title = next(
        (
            match.group(2).strip().rstrip("#").strip()
            for line in markdown.splitlines()
            if (match := _HEADING.match(line)) is not None
        ),
        Path(source_name).stem,
    )
    return (
        "---\n"
        f'title: "{_yaml_text(title)}"\n'
        f'source: "{_yaml_text(Path(source_name).name)}"\n'
        "generator: Liblevo\n"
        "---"
    )


def _split_chapters(markdown: str) -> tuple[str, tuple[MarkdownChapter, ...]]:
    lines = markdown.splitlines(keepends=True)
    if re.match(r"\A---[ \t]*\r?\n[\s\S]*?\r?\n(?:---|\.\.\.)[ \t]*(?:\r?\n|$)", markdown):
        return markdown, ()
    environment: dict[str, object] = {}
    tokens = _markdown_parser().parse(markdown, environment)
    # References and fragment links depend on the original document's scope. Keep that scope.
    if environment.get("references") or any(
        (
            isinstance(target := token.attrGet("href") or token.attrGet("src"), str)
            and target.startswith("#")
        )
        or (token.type.startswith("html") and re.search(r"\b(?:href|src)\s*=", token.content, re.I))
        for token in _walk_tokens(tokens)
    ):
        return markdown, ()
    headings = [
        (token.map[0], int(token.tag[1:]), tokens[index + 1].content)
        for index, token in enumerate(tokens)
        if token.type == "heading_open" and token.level == 0 and token.map is not None
    ]
    if not headings:
        return markdown, ()
    counts: dict[int, int] = {}
    for _, level, _ in headings:
        counts[level] = counts.get(level, 0) + 1
    split_level = next((level for level in sorted(counts) if counts[level] >= 2), None)
    if split_level is None:
        return markdown, ()
    heading_starts = [
        (position, title) for position, level, title in headings if level == split_level
    ]
    starts = [
        (_include_leading_page_marker(lines, position), title) for position, title in heading_starts
    ]
    if len(starts) < 2:
        return markdown, ()

    chapters: list[MarkdownChapter] = []
    preamble = "".join(lines[: starts[0][0]])
    for index, (start, title) in enumerate(starts):
        end = starts[index + 1][0] if index + 1 < len(starts) else len(lines)
        content = "".join(lines[start:end])
        if content:
            chapters.append(_chapter(len(chapters) + 1, title, content))
    return preamble, tuple(chapters)


def _include_leading_page_marker(lines: list[str], heading_position: int) -> int:
    position = heading_position - 1
    while position >= 0 and not lines[position].strip():
        position -= 1
    if position >= 0 and _PAGE_MARKER.fullmatch(lines[position].strip()):
        return position
    return heading_position


def _chapter(number: int, title: str, markdown: str) -> MarkdownChapter:
    normalized = unicodedata.normalize("NFKD", title).encode("ascii", "ignore").decode("ascii")
    slug = _UNSAFE_FILENAME.sub("-", normalized.casefold()).strip("-")[:64] or "capitulo"
    return MarkdownChapter(f"{number:02d}-{slug}.md", title, markdown)


def _yaml_text(value: str) -> str:
    return value.replace("\\", "\\\\").replace('"', '\\"').replace("\n", " ")


def _markdown_link_text(value: str) -> str:
    return value.replace("\\", "\\\\").replace("[", "\\[").replace("]", "\\]")


def _document_title(markdown: str) -> str | None:
    return next(
        (
            match.group(2).strip().rstrip("#").strip()
            for line in markdown.splitlines()
            if (match := _HEADING.match(line)) is not None
        ),
        None,
    )


def _join(prefix: str, markdown: str) -> str:
    parts = [part for part in (prefix.strip("\r\n"), markdown.strip("\r\n")) if part]
    return "\n\n".join(parts) + "\n"
