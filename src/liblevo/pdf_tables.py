"""PDF table geometry, conservative reconstruction and rendering."""

from __future__ import annotations

import math
import re
import unicodedata
from collections import Counter
from html import escape
from statistics import median
from typing import Any

from pdfplumber.utils.exceptions import PdfminerException
from PIL import Image

from liblevo.pdf_checkpoints import (
    _MAX_PDF_TABLE_CELL_CHARACTERS,
    _MAX_PDF_TABLE_COLUMNS,
    _MAX_PDF_TABLE_ROWS,
)
from liblevo.pdf_layout import (
    _MarkdownBlock,
    _PdfLine,
    _PdfTable,
    _RasterHorizontalRule,
    _TableRendering,
)
from liblevo.pdf_text_reconciliation import _hard_hyphen_wraps_word as _hard_hyphen_wraps_word
from liblevo.pdf_text_reconciliation import _heading_letter_count as _heading_letter_count
from liblevo.pdf_text_reconciliation import _is_bold_font as _is_bold_font
from liblevo.pdf_text_reconciliation import _is_uppercase_text as _is_uppercase_text
from liblevo.pdf_text_reconciliation import _normalize_text as _normalize_text

_LETTER_PATTERN = re.compile(r"[^\W\d_]", re.UNICODE)


_TABLE_CAPTION_LINE_PATTERN = re.compile(
    r"^(?:table|tabla|cuadro)\s+(?:\d+|[ivxlcdm]+)\s*[.\-:]",
    re.IGNORECASE,
)


_OPEN_RASTER_TABLE_MIN_AREA_RATIO = 0.08


_OPEN_RASTER_TABLE_MAX_AREA_RATIO = 0.75


_OPEN_RASTER_TABLE_MIN_SIDE_BY_SIDE_LINES = 8


_OPEN_RASTER_TABLE_MIN_SIDE_BY_SIDE_RATIO = 0.40


def _extract_tables(page: Any) -> tuple[_PdfTable, ...]:
    """Extract bounded tables and choose the least lossy portable representation."""

    if len(page.lines) + len(page.rects) < 4:
        return ()
    try:
        tables = page.find_tables()
    except (PdfminerException, TypeError, ValueError):
        return ()
    extracted: list[_PdfTable] = []
    for table in tables:
        data = table.extract() or []
        rows = tuple(tuple(_normalize_table_cell(cell) for cell in row) for row in data)
        column_count = max((len(row) for row in rows), default=0)
        populated_cells = sum(bool(cell) for row in rows for cell in row)
        if (
            len(rows) < 2
            or len(rows) > _MAX_PDF_TABLE_ROWS
            or column_count < 2
            or column_count > _MAX_PDF_TABLE_COLUMNS
            or populated_cells < 4
            or any(len(cell) > _MAX_PDF_TABLE_CELL_CHARACTERS for row in rows for cell in row)
        ):
            continue
        try:
            bbox = tuple(float(value) for value in table.bbox)
        except (TypeError, ValueError):
            continue
        if (
            len(bbox) != 4
            or not all(math.isfinite(value) for value in bbox)
            or bbox[0] < 0
            or bbox[1] < 0
            or bbox[0] >= bbox[2]
            or bbox[1] >= bbox[3]
            or bbox[2] > float(page.width)
            or bbox[3] > float(page.height)
        ):
            continue
        normalized_rows = tuple(row + ("",) * (column_count - len(row)) for row in rows)
        rendering = _table_rendering(normalized_rows, column_count)
        extracted.append(
            _PdfTable(
                (bbox[0], bbox[1], bbox[2], bbox[3]),
                normalized_rows,
                rendering,
            )
        )
    return tuple(extracted)


def _normalize_table_cell(value: object) -> str:
    if value is None:
        return ""
    normalized = str(value).replace("\r", "").replace("\u00ad\n", "").replace("\u00ad", "")
    return re.sub(r"[ \t]+", " ", normalized).strip()


def _render_table_cell(value: str) -> str:
    source_lines = value.split("\n")
    normalized_lines: list[str] = []
    for line in source_lines:
        if not normalized_lines:
            normalized_lines.append(line)
        elif normalized_lines[-1].endswith("-") and _hard_hyphen_wraps_word(
            normalized_lines[-1], line
        ):
            normalized_lines[-1] = f"{normalized_lines[-1][:-1]}{line.lstrip()}"
        elif _table_cell_line_is_soft_wrap(normalized_lines[-1], line):
            normalized_lines[-1] = f"{normalized_lines[-1].rstrip()} {line.lstrip()}"
        else:
            normalized_lines.append(line)
    return "\n".join(normalized_lines)


_TABLE_CELL_CONTINUATION_WORDS = frozenset(
    {
        "a",
        "an",
        "and",
        "at",
        "by",
        "con",
        "de",
        "del",
        "el",
        "en",
        "for",
        "from",
        "in",
        "la",
        "of",
        "on",
        "or",
        "para",
        "por",
        "the",
        "to",
        "with",
        "y",
    }
)


_TABLE_CELL_LIST_PREFIX_PATTERN = re.compile(r"^(?:[-*•‣▪◦]|\(?\d{1,3}[.)]|[A-ZÁÉÍÓÚÑ][.)])\s+")


def _table_cell_line_is_soft_wrap(previous: str, current: str) -> bool:
    """Join a visual PDF wrap without collapsing explicit items or sentences."""

    left = previous.rstrip()
    right = current.lstrip()
    if (
        not left
        or not right
        or left[-1] in ".!?;:"
        or _TABLE_CELL_LIST_PREFIX_PATTERN.match(right) is not None
    ):
        return False
    first_letter = next((character for character in right if character.isalpha()), "")
    if first_letter.islower() or left[-1] in ",([{—–":
        return True
    last_word = re.search(r"([^\W\d_]+)[’']?$", left, re.UNICODE)
    return bool(last_word and last_word.group(1).casefold() in _TABLE_CELL_CONTINUATION_WORDS)


def _table_rendering(
    rows: tuple[tuple[str, ...], ...],
    column_count: int,
) -> _TableRendering:
    longest = max((len(cell) for row in rows for cell in row), default=0)
    multiline = any("\n" in cell for row in rows for cell in row)
    if (
        len(rows) > 80
        or column_count > 16
        or longest > 2_000
        or _table_has_sparse_continuation_rows(rows, column_count)
    ):
        return _TableRendering.STRUCTURED_TEXT
    if column_count <= 8 and longest <= 220 and not multiline:
        return _TableRendering.MARKDOWN
    return _TableRendering.HTML


def _table_has_sparse_continuation_rows(
    rows: tuple[tuple[str, ...], ...],
    column_count: int,
) -> bool:
    """Flag grids whose apparent rows probably split a smaller set of visual records."""

    if column_count < 4 or len(rows) < 5:
        return False
    sparse_rows = sum(0 < sum(bool(cell) for cell in row) <= column_count - 2 for row in rows[1:])
    return sparse_rows >= 2


def _side_by_side_line_indexes(
    lines: tuple[_PdfLine, ...],
    page_width: float,
) -> set[int]:
    """Return lines participating in repeated horizontal cell relationships."""

    side_by_side: set[int] = set()
    minimum_gutter = page_width * 0.025
    for left_index, left in enumerate(lines):
        for right_index in range(left_index + 1, len(lines)):
            right = lines[right_index]
            vertical_overlap = min(left.bottom, right.bottom) - max(left.top, right.top)
            minimum_height = max(1.0, min(left.bottom - left.top, right.bottom - right.top))
            horizontally_separated = (
                left.x1 + minimum_gutter <= right.x0 or right.x1 + minimum_gutter <= left.x0
            )
            if vertical_overlap >= minimum_height * 0.35 and horizontally_separated:
                side_by_side.update((left_index, right_index))
    return side_by_side


def _open_raster_table_boundaries(
    page: Any,
    lines: tuple[_PdfLine, ...],
    page_width: float,
    page_height: float,
    rules: tuple[_RasterHorizontalRule, ...],
) -> tuple[tuple[float, ...], tuple[float, ...], bool] | None:
    """Recover a strict open-table grid from an ABBYY full-page scan."""

    if not 2 <= len(rules) <= 3:
        return None
    first = rules[0]
    last = rules[-1]
    widths = tuple(rule.x1 - rule.x0 for rule in rules)
    overlap = min(rule.x1 for rule in rules) - max(rule.x0 for rule in rules)
    if min(widths, default=0.0) <= 0 or overlap / min(widths) < 0.90:
        return None
    outer_left = float(median(rule.x0 for rule in rules))
    outer_right = float(median(rule.x1 for rule in rules))
    area_ratio = (outer_right - outer_left) * (last.top - first.top) / (page_width * page_height)
    if not _OPEN_RASTER_TABLE_MIN_AREA_RATIO <= area_ratio <= _OPEN_RASTER_TABLE_MAX_AREA_RATIO:
        return None

    table_lines = tuple(
        line for line in lines if first.top < (line.top + line.bottom) / 2 < last.top
    )
    side_by_side = _side_by_side_line_indexes(table_lines, page_width)
    if (
        len(side_by_side) < _OPEN_RASTER_TABLE_MIN_SIDE_BY_SIDE_LINES
        or len(side_by_side) / max(1, len(table_lines)) < _OPEN_RASTER_TABLE_MIN_SIDE_BY_SIDE_RATIO
    ):
        return None

    sparse_matrix = len(rules) == 3
    if sparse_matrix:
        vertical = _header_gutter_table_boundaries(table_lines, page_width, rules)
        if vertical is None:
            return None
    else:
        middle = _RasterHorizontalRule(
            outer_left,
            float((first.top + last.top) / 2),
            outer_right,
        )
        repeated = _spatial_table_boundaries(
            table_lines,
            side_by_side,
            page_width,
            page_height,
            (first, middle, last),
        )
        if repeated is None or len(repeated[0]) != 3:
            return None
        vertical = repeated[0]

    horizontal_result = _open_table_horizontal_boundaries(page, vertical, rules)
    if horizontal_result is None:
        return None
    horizontal, labelled_row_ratio = horizontal_result
    if len(horizontal) - 1 < 4:
        return None
    if not sparse_matrix and labelled_row_ratio < 0.75:
        return None
    return vertical, horizontal, sparse_matrix


def _header_gutter_table_boundaries(
    lines: tuple[_PdfLine, ...],
    page_width: float,
    rules: tuple[_RasterHorizontalRule, ...],
) -> tuple[float, ...] | None:
    """Infer matrix columns only from gutters repeated across two header lines."""

    if len(rules) != 3:
        return None
    candidates: list[tuple[float, float]] = []
    for line in lines:
        center_y = (line.top + line.bottom) / 2
        if not rules[0].top < center_y < rules[1].top:
            continue
        characters = sorted(
            (
                character
                for character in line.chars
                if character.text and not character.text.isspace()
            ),
            key=lambda character: character.x0,
        )
        minimum_gap = max(page_width * 0.012, line.font_size * 0.80)
        for previous, current in zip(characters, characters[1:], strict=False):
            if current.x0 - previous.x1 >= minimum_gap:
                candidates.append(((previous.x1 + current.x0) / 2, line.top))
    if not candidates:
        return None

    tolerance = page_width * 0.015
    clusters: list[list[tuple[float, float]]] = []
    for candidate in sorted(candidates):
        if not clusters or candidate[0] - median(item[0] for item in clusters[-1]) > tolerance:
            clusters.append([candidate])
        else:
            clusters[-1].append(candidate)
    internal = tuple(
        float(median(item[0] for item in cluster))
        for cluster in clusters
        if len({round(item[1], 1) for item in cluster}) >= 2
    )
    table_lines = tuple(
        line for line in lines if rules[0].top < (line.top + line.bottom) / 2 < rules[-1].top
    )
    vertical = (
        min(
            float(median(rule.x0 for rule in rules)),
            min((line.x0 for line in table_lines), default=page_width),
        ),
        *internal,
        max(
            float(median(rule.x1 for rule in rules)),
            max((line.x1 for line in table_lines), default=0.0),
        ),
    )
    if not 5 <= len(vertical) <= 9 or any(
        left >= right for left, right in zip(vertical, vertical[1:], strict=False)
    ):
        return None
    return vertical


def _open_table_horizontal_boundaries(
    page: Any,
    vertical: tuple[float, ...],
    rules: tuple[_RasterHorizontalRule, ...],
) -> tuple[tuple[float, ...], float] | None:
    """Split open-table rows only at clear whitespace in the first column."""

    inferred: list[float] = []
    row_starts: list[dict[str, Any]] = []
    for upper, lower in zip(rules, rules[1:], strict=False):
        try:
            first_column_crop = page.crop(
                (vertical[0], upper.top, vertical[1], lower.top),
                strict=False,
            )
            raw_lines = sorted(
                first_column_crop.extract_text_lines(strip=True, return_chars=True),
                key=lambda line: float(line["top"]),
            )
            full_width_lines = sorted(
                page.crop(
                    (vertical[0], upper.top, vertical[-1], lower.top),
                    strict=False,
                ).extract_text_lines(strip=True, return_chars=True),
                key=lambda line: float(line["top"]),
            )
        except (PdfminerException, KeyError, OSError, TypeError, ValueError):
            return None
        raw_lines = [line for line in raw_lines if _normalize_text(str(line.get("text", "")))]
        full_width_lines = [
            line for line in full_width_lines if _normalize_text(str(line.get("text", "")))
        ]
        if not raw_lines:
            return None
        sizes = [
            float(character.get("size", 0))
            for line in raw_lines
            for character in (line.get("chars") or ())
            if float(character.get("size", 0)) > 0
        ]
        minimum_gap = max(3.5, (median(sizes) if sizes else 7.0) * 0.55)
        segment_starts = [raw_lines[0]]
        for previous, current in zip(raw_lines, raw_lines[1:], strict=False):
            gap = float(current["top"]) - float(previous["bottom"])
            if gap >= minimum_gap:
                segment_starts.append(current)
        row_starts.extend(segment_starts)
        row_tolerance = max(2.0, (median(sizes) if sizes else 7.0) * 0.35)
        for current, following in zip(segment_starts, segment_starts[1:], strict=False):
            current_top = float(current["top"])
            following_top = float(following["top"])
            previous_bottom = max(
                (
                    float(line["bottom"])
                    for line in full_width_lines
                    if float(line["top"]) >= current_top - row_tolerance
                    and float(line["top"]) < following_top - 1.0
                ),
                default=float(current["bottom"]),
            )
            if previous_bottom >= following_top:
                return None
            inferred.append(float((previous_bottom + following_top) / 2))

    horizontal = tuple(sorted((*(rule.top for rule in rules), *inferred)))
    if len(horizontal) - 1 > _MAX_PDF_TABLE_ROWS or any(
        top >= bottom for top, bottom in zip(horizontal, horizontal[1:], strict=False)
    ):
        return None
    labelled = sum(_raw_table_line_is_label(line) for line in row_starts)
    return horizontal, labelled / max(1, len(row_starts))


def _restore_visual_matrix_placeholders(
    page: Any,
    rows: tuple[tuple[str, ...], ...],
    vertical: tuple[float, ...],
    horizontal: tuple[float, ...],
) -> tuple[tuple[str, ...], ...]:
    """Restore dash placeholders visible in the scan but absent from hidden OCR text."""

    if (
        len(rows) + 1 != len(horizontal)
        or not rows
        or any(len(row) + 1 != len(vertical) for row in rows)
    ):
        return rows
    try:
        image = page.to_image(resolution=144, antialias=True).original.convert("L")
        page_width = float(page.width)
        page_height = float(page.height)
    except (OSError, TypeError, ValueError):
        return rows
    if page_width <= 0 or page_height <= 0:
        return rows

    scale_x = image.width / page_width
    scale_y = image.height / page_height
    restored = [list(row) for row in rows]
    for row_index in range(1, len(rows)):
        for column_index in range(1, len(rows[row_index])):
            if rows[row_index][column_index]:
                continue
            bbox = (
                vertical[column_index],
                horizontal[row_index],
                vertical[column_index + 1],
                horizontal[row_index + 1],
            )
            if _raster_cell_has_short_horizontal_mark(image, bbox, scale_x, scale_y):
                restored[row_index][column_index] = "—"
    return tuple(tuple(row) for row in restored)


def _repair_consensus_degree_markers(
    rows: tuple[tuple[str, ...], ...],
) -> tuple[tuple[str, ...], ...]:
    """Repair one hidden-OCR degree glyph only when peer headers establish the notation."""

    if not rows or len(rows[0]) < 4:
        return rows
    header = rows[0]
    confirmed = sum(bool(re.search(r"(?<!\d)\d{1,2}°\s*[A-Z]{3,}\b", cell)) for cell in header[1:])
    if confirmed < max(2, (len(header) - 1) // 2):
        return rows

    repaired_header: list[str] = []
    for cell in header:
        repaired = re.sub(
            r"(?<!\d)(?P<degree>[0-2]?\d)0(?=\s*[A-Z]{3,}\b)",
            lambda match: f"{match.group('degree')}°",
            cell,
        )
        repaired_header.append(re.sub(r"(?<=°)(?=[A-Z])", " ", repaired))
    if tuple(repaired_header) == header:
        return rows
    return (tuple(repaired_header), *rows[1:])


def _raster_cell_has_short_horizontal_mark(
    image: Image.Image,
    bbox: tuple[float, float, float, float],
    scale_x: float,
    scale_y: float,
) -> bool:
    """Recognize one small dash component while excluding the table's outer rules."""

    x0, top, x1, bottom = bbox
    inset_x = min(2.0, max(0.75, (x1 - x0) * 0.025))
    inset_y = min(2.0, max(0.75, (bottom - top) * 0.025))
    crop_box = (
        max(0, round((x0 + inset_x) * scale_x)),
        max(0, round((top + inset_y) * scale_y)),
        min(image.width, round((x1 - inset_x) * scale_x)),
        min(image.height, round((bottom - inset_y) * scale_y)),
    )
    if crop_box[2] <= crop_box[0] or crop_box[3] <= crop_box[1]:
        return False
    crop = image.crop(crop_box).convert("L")
    width, height = crop.size
    pixels = crop.tobytes()
    visited: set[tuple[int, int]] = set()
    for y in range(height):
        for x in range(width):
            if (x, y) in visited or pixels[y * width + x] >= 155:
                continue
            pending = [(x, y)]
            visited.add((x, y))
            component: list[tuple[int, int]] = []
            while pending:
                current_x, current_y = pending.pop()
                component.append((current_x, current_y))
                for delta_y in (-1, 0, 1):
                    for delta_x in (-1, 0, 1):
                        neighbour = (current_x + delta_x, current_y + delta_y)
                        if (
                            0 <= neighbour[0] < width
                            and 0 <= neighbour[1] < height
                            and neighbour not in visited
                            and pixels[neighbour[1] * width + neighbour[0]] < 155
                        ):
                            visited.add(neighbour)
                            pending.append(neighbour)
            component_width = (
                max(point[0] for point in component) - min(point[0] for point in component) + 1
            )
            component_height = (
                max(point[1] for point in component) - min(point[1] for point in component) + 1
            )
            density = len(component) / (component_width * component_height)
            if (
                6 <= component_width <= max(8, round(width * 0.45))
                and component_height <= max(5, round(component_width * 0.25))
                and component_width >= component_height * 3
                and len(component) >= component_width * 0.60
                and density >= 0.30
            ):
                return True
    return False


def _raw_table_line_is_label(raw_line: dict[str, Any]) -> bool:
    text = _normalize_text(str(raw_line.get("text", "")))
    if _is_uppercase_text(text):
        return True
    characters = tuple(raw_line.get("chars") or ())
    weighted = sum(max(1, len(str(character.get("text", "")))) for character in characters)
    bold = sum(
        max(1, len(str(character.get("text", ""))))
        for character in characters
        if _is_bold_font(str(character.get("fontname", "")))
    )
    return weighted > 0 and bold / weighted >= 0.55


def _complete_sparse_table_rules(
    rules: tuple[_RasterHorizontalRule, ...],
    lines: tuple[_PdfLine, ...],
    page_width: float,
) -> tuple[_RasterHorizontalRule, ...] | None:
    """Infer missing inner rules for a short, explicitly captioned table fragment."""

    if len(rules) != 2:
        return None
    side_by_side = _side_by_side_line_indexes(lines, page_width)
    aligned = sorted((lines[index] for index in side_by_side), key=lambda line: line.x0)
    if len(aligned) < 6:
        return None
    tolerance = page_width * 0.045
    first_column = [aligned[0]]
    for line in aligned[1:]:
        if line.x0 - median(item.x0 for item in first_column) > tolerance:
            break
        first_column.append(line)
    row_lines = sorted(
        (
            line
            for line in first_column
            if rules[0].top < (line.top + line.bottom) / 2 < rules[-1].top
        ),
        key=lambda line: line.top,
    )
    starts: list[float] = []
    for line in row_lines:
        if not starts or line.top - starts[-1] > max(2.0, line.font_size * 0.45):
            starts.append(line.top)
    if not 3 <= len(starts) <= _MAX_PDF_TABLE_ROWS:
        return None
    boundaries: list[float] = []
    for current, following in zip(starts, starts[1:], strict=False):
        row_tolerance = max(
            4.0,
            median(line.font_size for line in row_lines) * 0.75,
        )
        next_top = min(
            (
                line.top
                for line in lines
                if following - row_tolerance <= line.top <= following + row_tolerance
            ),
            default=following,
        )
        previous_bottom = max(
            (
                line.bottom
                for line in lines
                if line.top >= current - row_tolerance and line.top < next_top - 1.0
            ),
            default=current,
        )
        if previous_bottom >= next_top:
            return None
        boundaries.append(float((previous_bottom + next_top) / 2))
    x0 = float(median(rule.x0 for rule in rules))
    x1 = float(median(rule.x1 for rule in rules))
    return (
        rules[0],
        *(_RasterHorizontalRule(x0, top, x1) for top in boundaries),
        rules[-1],
    )


def _spatial_table_rule_groups(
    rules: tuple[_RasterHorizontalRule, ...],
    lines: tuple[_PdfLine, ...],
) -> tuple[tuple[_RasterHorizontalRule, ...], ...]:
    """Split consecutive ruled tables only at an explicit caption between their rule sets."""

    if not rules:
        return ()
    split_after: set[int] = set()
    for index, (upper, lower) in enumerate(zip(rules, rules[1:], strict=False)):
        if index + 1 < 3 or len(rules) - index - 1 < 2:
            continue
        if any(
            upper.top < (line.top + line.bottom) / 2 < lower.top
            and _TABLE_CAPTION_LINE_PATTERN.match(line.text.strip())
            for line in lines
        ):
            split_after.add(index)
    groups: list[tuple[_RasterHorizontalRule, ...]] = []
    start = 0
    for index in sorted(split_after):
        groups.append(rules[start : index + 1])
        start = index + 1
    groups.append(rules[start:])
    return tuple(groups)


def _raster_horizontal_rules(
    page: Any,
    *,
    minimum_width_ratio: float = 0.70,
) -> tuple[_RasterHorizontalRule, ...]:
    """Locate long horizontal rules that exist only in the rendered page image."""

    try:
        image = page.to_image(resolution=144, antialias=True).original.convert("L")
    except (OSError, TypeError, ValueError):
        return ()
    width, height = image.size
    if width <= 0 or height <= 0:
        return ()
    data = image.tobytes()
    grouped: list[list[tuple[int, tuple[int, int]]]] = []
    current: list[tuple[int, tuple[int, int]]] = []
    for y in range(height + 1):
        span = (
            _long_dark_horizontal_span(
                data,
                width,
                height,
                y,
                minimum_width_ratio=minimum_width_ratio,
            )
            if y < height
            else None
        )
        if span is not None and y / height < 0.97:
            current.append((y, span))
        elif current:
            grouped.append(current)
            current = []
    scale_x = float(page.width) / width
    scale_y = float(page.height) / height
    return tuple(
        _RasterHorizontalRule(
            float(median(span[0] for _y, span in group)) * scale_x,
            float(median(y for y, _span in group)) * scale_y,
            float(median(span[1] for _y, span in group)) * scale_x,
        )
        for group in grouped
    )


def _long_dark_horizontal_span(
    data: bytes,
    width: int,
    height: int,
    y: int,
    *,
    minimum_width_ratio: float = 0.70,
) -> tuple[int, int] | None:
    dark_x = bytearray(width)
    for scan_y in range(max(0, y - 2), min(height, y + 3)):
        row = data[scan_y * width : (scan_y + 1) * width]
        for x, value in enumerate(row):
            if value < 200:
                dark_x[x] = 1
    best: tuple[int, int] | None = None
    start: int | None = None
    previous = -1
    gap = 0
    for x, is_dark in enumerate(dark_x):
        if is_dark:
            if start is None or gap > 3:
                if start is not None and (best is None or previous + 1 - start > best[1] - best[0]):
                    best = (start, previous + 1)
                start = x
            previous = x
            gap = 0
        elif start is not None:
            gap += 1
    if start is not None and (best is None or previous + 1 - start > best[1] - best[0]):
        best = (start, previous + 1)
    if best is None or best[1] - best[0] < width * minimum_width_ratio:
        return None
    return best


def _spatial_table_boundaries(
    lines: tuple[_PdfLine, ...],
    side_by_side: set[int],
    page_width: float,
    page_height: float,
    rules: tuple[_RasterHorizontalRule, ...],
    *,
    allow_singleton_columns: bool = False,
) -> tuple[tuple[float, ...], tuple[float, ...]] | None:
    if len(rules) < 3:
        return None
    tolerance = page_width * 0.045
    aligned = sorted((lines[index] for index in side_by_side), key=lambda line: line.x0)
    clusters: list[list[_PdfLine]] = []
    for line in aligned:
        if not clusters or line.x0 - median(item.x0 for item in clusters[-1]) > tolerance:
            clusters.append([line])
        else:
            clusters[-1].append(line)
    clusters = [cluster for cluster in clusters if len(cluster) >= 2 or allow_singleton_columns]
    if not 2 <= len(clusters) <= 8:
        return None
    anchors = tuple(float(median(line.x0 for line in cluster)) for cluster in clusters)
    table_row_lines = tuple(
        line for line in lines if rules[0].top < (line.top + line.bottom) / 2 < rules[-1].top
    )
    internal_boundaries: list[float] = []
    minimum_gutter = 0.01
    for left_cluster, right_cluster in zip(clusters, clusters[1:], strict=False):
        substantial_right_lines = tuple(
            line for line in right_cluster if _heading_letter_count(line.text) >= 3
        )
        right_edge = min(line.x0 for line in (substantial_right_lines or tuple(right_cluster)))
        character_edges = [
            character.x1
            for line in table_row_lines
            for character in line.chars
            if character.x0 < right_edge and character.x1 <= right_edge
        ]
        left_edge = (
            max(character_edges) if character_edges else max(line.x1 for line in left_cluster)
        )
        if right_edge - left_edge < minimum_gutter:
            return None
        internal_boundaries.append(float((left_edge + right_edge) / 2))
    outer_left = min(
        float(median(rule.x0 for rule in rules)),
        min((line.x0 for line in table_row_lines), default=page_width),
    )
    outer_right = max(
        float(median(rule.x1 for rule in rules)),
        max((line.x1 for line in table_row_lines), default=0.0),
    )
    vertical = (
        outer_left,
        *internal_boundaries,
        outer_right,
    )
    if any(left >= right for left, right in zip(vertical, vertical[1:], strict=False)):
        return None
    horizontal = [rule.top for rule in rules]
    first = horizontal[0]
    near_above = tuple(
        line
        for line in lines
        if not line.rotated
        and line.bottom <= first + 1
        and line.top >= max(0.0, first - page_height * 0.10)
        and _LETTER_PATTERN.search(line.text)
    )
    aligned_columns = {
        min(range(len(anchors)), key=lambda index: abs(line.x0 - anchors[index]))
        for line in near_above
        if min(abs(line.x0 - anchor) for anchor in anchors) <= page_width * 0.025
    }
    if len(_side_by_side_line_indexes(near_above, page_width)) >= 2 and len(aligned_columns) >= 2:
        horizontal.insert(0, min(line.top for line in near_above))
    if len(horizontal) < 3 or any(
        top >= bottom for top, bottom in zip(horizontal, horizontal[1:], strict=False)
    ):
        return None
    return vertical, tuple(horizontal)


def _table_has_exact_character_coverage(
    page: Any,
    bbox: tuple[float, float, float, float],
    rows: tuple[tuple[str, ...], ...],
) -> bool:
    x0, top, x1, bottom = bbox
    source = "".join(
        str(character.get("text", ""))
        for character in page.chars
        if x0 <= (float(character["x0"]) + float(character["x1"])) / 2 < x1
        and top <= (float(character["top"]) + float(character["bottom"])) / 2 < bottom
    )
    extracted = "".join(cell for row in rows for cell in row)
    return _significant_character_counts(source) == _significant_character_counts(extracted)


def _significant_character_counts(value: str) -> Counter[str]:
    return Counter(
        character
        for character in unicodedata.normalize("NFKC", value).casefold()
        if unicodedata.category(character)[0] in {"L", "N", "P", "S"}
    )


def _append_pdf_table(
    blocks: list[_MarkdownBlock],
    page_number: int,
    table: _PdfTable,
) -> None:
    if table.rendering is _TableRendering.MARKDOWN:
        markdown = _markdown_table(table.rows)
    elif table.rendering is _TableRendering.HTML:
        markdown = _html_table(table.rows)
    else:
        markdown = _structured_table_text(table.rows)
        blocks.append(
            _MarkdownBlock(
                kind="warning",
                text=(
                    f"> **Aviso de conversión (página {page_number}):** la tabla es demasiado "
                    "compleja para representarla con seguridad. Se ha conservado como texto "
                    "estructurado; compárala con el original."
                ),
                page_number=page_number,
            )
        )
    blocks.append(_MarkdownBlock(kind="raw", text=markdown, page_number=page_number))


def _markdown_table(rows: tuple[tuple[str, ...], ...]) -> str:
    header = tuple(_render_table_cell(cell) for cell in rows[0])

    def row(cells: tuple[str, ...]) -> str:
        return (
            "| " + " | ".join(_render_table_cell(cell).replace("|", "\\|") for cell in cells) + " |"
        )

    return "\n".join(
        (row(header), row(tuple("---" for _ in header)), *(row(item) for item in rows[1:]))
    )


def _html_table(rows: tuple[tuple[str, ...], ...]) -> str:
    header = "".join(f"<th>{escape(_render_table_cell(cell))}</th>" for cell in rows[0])
    body = "".join(
        "<tr>"
        + "".join(
            f"<td>{escape(_render_table_cell(cell)).replace(chr(10), '<br>')}</td>" for cell in row
        )
        + "</tr>"
        for row in rows[1:]
    )
    return f"<table>\n<thead><tr>{header}</tr></thead>\n<tbody>{body}</tbody>\n</table>"


def _structured_table_text(rows: tuple[tuple[str, ...], ...]) -> str:
    headers = tuple(cell or f"Columna {index}" for index, cell in enumerate(rows[0], start=1))
    rendered = ["**Tabla recuperada**"]
    for row_number, row in enumerate(rows[1:], start=1):
        cells = "; ".join(
            f"{header}: {_render_table_cell(value)}"
            for header, value in zip(headers, row, strict=True)
            if value
        )
        rendered.append(f"- Fila {row_number}: {cells or 'sin contenido'}")
    return "\n".join(rendered)
