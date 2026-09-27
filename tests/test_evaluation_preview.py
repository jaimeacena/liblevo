from pathlib import Path

from lxml import html
from scripts.evaluation_preview import document_preview, passive_html


def test_markdown_and_html_tables_render_as_passive_readable_content(tmp_path: Path):
    source = tmp_path / "result.md"
    source.write_text(
        "# Heading\n\nA **bold** word.\n\n| Item | Count |\n| --- | --- |\n| A | 2 |\n\n"
        '<table><tr><td rowspan="2">B</td><td>3</td></tr><tr><td>4</td></tr></table>',
        encoding="utf-8",
    )
    rendered, limited = document_preview(source)
    tree = html.fromstring(rendered)
    assert not limited
    assert tree.xpath("//h1/text()") == ["Heading"]
    assert tree.xpath("//strong/text()") == ["bold"]
    assert len(tree.xpath("//table")) == 2
    assert tree.xpath("//td[@rowspan='2']/text()") == ["B"]


def test_preview_blocks_active_content_urls_css_and_dom_clobbering(tmp_path: Path):
    rendered, limited = passive_html(
        '<p id="copy" onclick="alert(1)" style="background:url(https://invalid/)">Before'
        "<!-- page marker --> after<script>alert(1)</script> end</p>"
        '<a href="javascript:alert(1)">Link</a><iframe src="https://invalid/"></iframe>'
        '<img src="https://invalid/image.png" onerror="alert(1)">'
        '<form><input name="data"></form><svg onload="alert(1)"></svg>',
        tmp_path,
    )
    tree = html.fromstring(rendered)
    assert limited
    assert tree.xpath("//p/text()") == ["Before after end"]
    assert not tree.xpath("//script|//iframe|//form|//input|//svg")
    assert not tree.xpath("//@id|//@name|//@style|//@onclick|//@onerror|//@href|//@src")
    assert "https://" not in rendered


def test_preview_only_embeds_verified_local_raster_images(tmp_path: Path):
    from PIL import Image

    Image.new("RGB", (10, 10), "white").save(tmp_path / "allowed.png")
    rendered, limited = passive_html('<img src="allowed.png" alt="Local">', tmp_path)
    assert not limited
    assert html.fromstring(rendered).xpath("//img/@src")[0].startswith("data:image/png;base64,")
    for value in ("../private.png", "file:///private.png", "data:image/png;base64,abc"):
        rendered, limited = passive_html(f'<img src="{value}">', tmp_path)
        assert limited
        assert not html.fromstring(rendered).xpath("//@src")


def test_long_preview_is_explicitly_partial(tmp_path: Path):
    source = tmp_path / "long.txt"
    source.write_text("a" * 80001, encoding="utf-8")
    rendered, limited = document_preview(source)
    assert limited
    assert len(html.fromstring(rendered).text_content()) == 80000


def test_original_pdf_is_rendered_locally_as_a_page_image(tmp_path: Path):
    from scripts.evaluation_preview import original_preview

    from pdf_canaries import write_canary_suite

    source = write_canary_suite(tmp_path / "sources")["contents"]
    rendered, limited = original_preview(source, [1, 1], tmp_path, 0)
    assert not limited
    image = html.fromstring(rendered).xpath("//img/@src")[0]
    assert (tmp_path / image).read_bytes().startswith(b"\x89PNG")
    assert "Página 1 del PDF" in rendered
