"""Reconcile native PDF and OCR text only with corroborating document evidence."""

from __future__ import annotations

import logging
import re
import unicodedata
from collections import defaultdict
from dataclasses import replace
from difflib import SequenceMatcher
from statistics import median

from parsezen.pdf_layout import _PdfLine, _PdfPage, _PdfTable

LOGGER = logging.getLogger("parsezen.pdf_conversion")

_WHITESPACE_PATTERN = re.compile(r"[\t \u00a0]+")


_SPACED_WORD_PATTERN = re.compile(
    r"(?<!\w)(?:[^\W\d_]\s+){4,}[^\W\d_](?!\w)",
    re.UNICODE,
)


_PARTIAL_SPACED_WORD_PATTERN = re.compile(
    r"(?<!\w)[^\W\d_]{2,3}(?:\s+[^\W\d_]){3,}(?!\w)",
    re.UNICODE,
)


_DEGREE_SHAPED_NATIVE_ZERO_PATTERN = re.compile(r"(?<!\d)(?P<degree>[0-2]?\d)0(?=$|[\s.,;:!?)\]])")


_ZODIAC_SIGN_NAMES = frozenset(
    {
        "Acuario",
        "Acquario",
        "Aquarius",
        "Aquário",
        "Aries",
        "Ariete",
        "Áries",
        "Balance",
        "Balança",
        "Bélier",
        "Cancer",
        "Cancro",
        "Cáncer",
        "Capricorn",
        "Capricorne",
        "Capricornio",
        "Capricórnio",
        "Capricorno",
        "Escorpio",
        "Escorpião",
        "Fische",
        "Gemelli",
        "Gémeaux",
        "Géminis",
        "Gemini",
        "Geminis",
        "Gêmeos",
        "Jungfrau",
        "Krebs",
        "Leão",
        "Leo",
        "Leone",
        "Libra",
        "Lion",
        "Löwe",
        "Peixes",
        "Pesci",
        "Pisces",
        "Piscis",
        "Poissons",
        "Sagittaire",
        "Sagitario",
        "Sagitário",
        "Sagittario",
        "Sagittarius",
        "Schütze",
        "Scorpio",
        "Scorpion",
        "Scorpione",
        "Skorpion",
        "Steinbock",
        "Stier",
        "Taureau",
        "Taurus",
        "Toro",
        "Touro",
        "Tauro",
        "Verseau",
        "Vierge",
        "Virgem",
        "Virgo",
        "Waage",
        "Wassermann",
        "Widder",
        "Zwillinge",
    }
)


_ZODIAC_SIGN_PATTERN = (
    "(?:"
    + "|".join(re.escape(name) for name in sorted(_ZODIAC_SIGN_NAMES, key=len, reverse=True))
    + ")"
)


_INVALID_ZODIAC_DEGREE_ZERO_PATTERN = re.compile(
    rf"(?<!\d)(?P<degree>[3-9]\d)0(?=\s+{_ZODIAC_SIGN_PATTERN}\b)",
    re.IGNORECASE,
)


_SUSPICIOUS_NUMERIC_GLYPH_PATTERN = re.compile(
    r"(?<!\w)(?:"
    r"(?=[\d$^]{2,6}(?!\w))(?=[\d$^]*[$^])[\d$^]{2,6}"
    r"|(?=[\dA-Za-z]{2,7}(?!\w))(?=[\dA-Za-z]*\d)(?=[\dA-Za-z]*[A-Za-z])"
    r"[\dA-Za-z]{2,7}"
    r")(?!\w)"
)


_NUMERIC_GLYPH_EXPANSIONS: dict[str, tuple[str, ...]] = {
    "I": ("1",),
    "i": ("1",),
    "l": ("1",),
    "O": ("0",),
    "o": ("0",),
    "H": ("11",),
    "h": ("11",),
    "n": ("11",),
    "S": ("5", "8"),
    "s": ("5", "8"),
    "B": ("8",),
    "b": ("8",),
}


_VISUAL_ATOM_PATTERN = re.compile(
    r"[\d$^][\d$^A-Za-z]{1,6}(?=(?:[.)](?:\s|$)|\s|$))"
    r"|[\d$^\ufffd°]+(?:[.,][\d$^\ufffd°]+)*"
    r"|(?:[^\W\d_]|\ufffd)+(?:[’'\-](?:[^\W\d_]|\ufffd)+)*"
    r"|[^\w\s]",
    re.UNICODE,
)


_LITERAL_URL_PATTERN = re.compile(r"(?:https?://|www\.)\S+", re.IGNORECASE)


def _repair_native_degree_markers(pages: list[_PdfPage]) -> tuple[list[_PdfPage], int]:
    """Restore a superscript degree glyph misencoded as zero from native geometry."""

    accepted = 0
    repaired_pages: list[_PdfPage] = []
    for page in pages:
        repaired_lines: list[_PdfLine] = []
        for line in page.lines:
            candidates = tuple(
                sorted(
                    (
                        *_DEGREE_SHAPED_NATIVE_ZERO_PATTERN.finditer(line.text),
                        *_INVALID_ZODIAC_DEGREE_ZERO_PATTERN.finditer(line.text),
                    ),
                    key=lambda match: match.start(),
                )
            )
            zero_characters = tuple(character for character in line.chars if character.text == "0")
            visible_sizes = tuple(
                character.size
                for character in line.chars
                if character.text.strip() and character.size > 0
            )
            if not candidates or not visible_sizes:
                repaired_lines.append(line)
                continue
            typical_size = median(visible_sizes)
            superscript_zeroes = tuple(
                character
                for character in zero_characters
                if character.size <= typical_size * 0.80
                and line.bottom - character.bottom >= max(1.0, typical_size * 0.18)
            )
            if not (len(candidates) == len(zero_characters) == len(superscript_zeroes)):
                repaired_lines.append(line)
                continue
            repaired_text = _DEGREE_SHAPED_NATIVE_ZERO_PATTERN.sub(
                lambda match: f"{match.group('degree')}°",
                line.text,
            )
            repaired_text = _INVALID_ZODIAC_DEGREE_ZERO_PATTERN.sub(
                lambda match: f"{match.group('degree')}°",
                repaired_text,
            )
            repaired_lines.append(replace(line, text=repaired_text, chars=()))
            accepted += len(candidates)
        repaired_pages.append(replace(page, lines=tuple(repaired_lines)))
    if accepted:
        LOGGER.info("pdf_native_degree_geometry_completed accepted=%d", accepted)
    return repaired_pages, accepted


def _spacing_insensitive_line_key(text: str) -> str:
    normalized = unicodedata.normalize("NFKC", text).casefold()
    return re.sub(r"\s+", "", normalized)


def _reconcile_suspicious_toc_numbers(
    lines: list[_PdfLine],
    ocr_markdown: str | None,
) -> list[_PdfLine]:
    """Decode broken numeric glyphs only with independent or sequential evidence.

    The selectable layer remains authoritative for every letter and for layout. A suspicious
    token changes only when local OCR or neighbouring clean folios leave one numeric reading.
    """

    ocr_numbers = set(re.findall(r"(?<!\d)\d{1,6}(?!\d)", ocr_markdown or ""))
    native_folios = tuple(
        (line, int(compact))
        for line in lines
        for compact in (re.sub(r"\s+", "", line.text),)
        if re.fullmatch(r"\d{1,6}", compact)
    )
    has_trailing_native_folios = any(
        re.search(r"(?<!\w)\d{1,4}\s*$", line.text) is not None for line in lines
    )
    if not ocr_numbers and not native_folios and not has_trailing_native_folios:
        return lines

    ocr_lines = tuple(line for line in (ocr_markdown or "").splitlines() if line.strip())

    def lexical_key(value: str) -> str:
        words = re.findall(r"[^\W\d_]{2,}", value.casefold(), re.UNICODE)
        return " ".join(words)

    def line_ocr_numbers(source_line: str) -> set[str]:
        source_key = lexical_key(source_line)
        if not source_key:
            return ocr_numbers
        ranked = sorted(
            (
                SequenceMatcher(None, source_key, lexical_key(candidate), autojunk=False).ratio(),
                candidate,
            )
            for candidate in ocr_lines
            if lexical_key(candidate)
        )
        if not ranked or ranked[-1][0] < 0.72:
            return ocr_numbers
        best_score = ranked[-1][0]
        best_lines = [candidate for score, candidate in ranked if score >= best_score - 0.02]
        return set(re.findall(r"(?<!\d)\d{1,6}(?!\d)", "\n".join(best_lines))) or ocr_numbers

    def ocr_evidence_text(source_line: _PdfLine) -> str:
        """Attach a detached folio to its visual label before matching OCR text."""

        if lexical_key(source_line.text):
            return source_line.text
        row_labels = [
            candidate
            for candidate in lines
            if candidate is not source_line
            and candidate.x0 < source_line.x0
            and _heading_letter_count(candidate.text) >= 2
            and _toc_lines_share_row(candidate, source_line)
        ]
        if not row_labels:
            return source_line.text
        label = min(
            row_labels,
            key=lambda candidate: (
                abs((candidate.top + candidate.bottom) - (source_line.top + source_line.bottom)),
                -candidate.x1,
            ),
        )
        return f"{label.text} {source_line.text}"

    def native_sequence_numbers(source_line: _PdfLine) -> set[str]:
        """Constrain one detached broken folio by its neighbours in the same column."""

        token = re.sub(r"\s+", "", source_line.text)
        if _SUSPICIOUS_NUMERIC_GLYPH_PATTERN.fullmatch(token) is None:
            return set()
        column_tolerance = source_line.page_width * 0.08
        preceding = [
            (line, value)
            for line, value in native_folios
            if line.top < source_line.top - 0.5
            and abs(line.x0 - source_line.x0) <= column_tolerance
        ]
        following = [
            (line, value)
            for line, value in native_folios
            if line.top > source_line.top + 0.5
            and abs(line.x0 - source_line.x0) <= column_tolerance
        ]
        if not preceding or not following:
            return set()
        lower = max(preceding, key=lambda item: item[0].top)[1]
        upper = min(following, key=lambda item: item[0].top)[1]
        if lower > upper:
            return set()
        return {
            candidate
            for candidate in _numeric_glyph_candidates(token)
            if lower <= int(candidate) <= upper
        }

    def reconcile_token(match: re.Match[str], confirmed_numbers: set[str]) -> str:
        token = match.group(0)
        candidates = _numeric_glyph_candidates(token)
        if not candidates:
            return token
        # A trailing dollar sign can be a real currency marker.  Without a following ordinal
        # separator or another digit it remains visible and is reported for review.
        if "$" in token and match.end() == len(match.string):
            return token
        confirmed = candidates & confirmed_numbers
        return next(iter(confirmed)) if len(confirmed) == 1 else token

    reconciled: list[_PdfLine] = []
    for line_index, line in enumerate(lines):
        native_sequence = native_sequence_numbers(line)
        ocr_confirmed_numbers = line_ocr_numbers(ocr_evidence_text(line))
        native_ocr_consensus = native_sequence & ocr_confirmed_numbers
        confirmed_numbers = native_ocr_consensus or native_sequence or ocr_confirmed_numbers

        def reconcile_with_order(
            match: re.Match[str],
            confirmed: set[str] = confirmed_numbers,
            current_line_index: int = line_index,
        ) -> str:
            independently_confirmed = reconcile_token(match, confirmed)
            if independently_confirmed != match.group(0):
                return independently_confirmed
            ordered = _ordered_toc_folio_candidates(lines, current_line_index, match)
            return reconcile_token(match, ordered)

        text = _SUSPICIOUS_NUMERIC_GLYPH_PATTERN.sub(reconcile_with_order, line.text)
        reconciled.append(replace(line, text=text) if text != line.text else line)
    return reconciled


def _reconcile_toc_numbers_with_native_priority(
    lines: list[_PdfLine],
    ocr_markdown: str | None,
) -> list[_PdfLine]:
    """Prefer a unique native folio sequence over conflicting whole-page OCR."""

    native_reconciled = _reconcile_suspicious_toc_numbers(lines, None)
    if not ocr_markdown:
        return native_reconciled
    ocr_reconciled = _reconcile_suspicious_toc_numbers(lines, ocr_markdown)
    return [
        native_candidate if native_candidate.text != source.text else ocr_candidate
        for source, native_candidate, ocr_candidate in zip(
            lines,
            native_reconciled,
            ocr_reconciled,
            strict=True,
        )
    ]


def _numeric_glyph_candidates(token: str) -> set[str]:
    """Expand common broken-font number shapes without selecting one by itself."""

    candidates = {""}
    has_uncertain_shape = False
    for character in token:
        replacements: tuple[str, ...]
        if character.isdigit():
            replacements = (character,)
        elif character in _NUMERIC_GLYPH_EXPANSIONS:
            replacements = _NUMERIC_GLYPH_EXPANSIONS[character]
            has_uncertain_shape = True
        elif character in "$^" or character.isalpha():
            replacements = tuple("0123456789")
            has_uncertain_shape = True
        else:
            return set()
        candidates = {
            f"{prefix}{replacement}" for prefix in candidates for replacement in replacements
        }
        if len(candidates) > 1_000:
            return set()
    return {
        candidate
        for candidate in candidates
        if has_uncertain_shape
        and candidate.isdecimal()
        and 1 <= len(candidate) <= 4
        and not candidate.startswith("0")
        and 1 <= int(candidate) <= 9_999
    }


def _ordered_toc_folio_candidates(
    lines: list[_PdfLine],
    line_index: int,
    match: re.Match[str],
) -> set[str]:
    """Use neighbouring clean TOC folios to select a broken trailing number."""

    if match.end() != len(match.string):
        return set()
    candidates = _numeric_glyph_candidates(match.group(0))
    if not candidates:
        return set()

    def trailing_folio(line: _PdfLine) -> int | None:
        found = re.search(r"(?<!\w)(\d{1,4})\s*$", line.text)
        return int(found.group(1)) if found is not None else None

    lower = next(
        (
            value
            for candidate_line in reversed(lines[:line_index])
            for value in (trailing_folio(candidate_line),)
            if value is not None
        ),
        None,
    )
    upper = next(
        (
            value
            for candidate_line in lines[line_index + 1 :]
            for value in (trailing_folio(candidate_line),)
            if value is not None
        ),
        None,
    )
    if lower is not None and upper is not None and lower <= upper:
        return {candidate for candidate in candidates if lower <= int(candidate) <= upper}
    neighbour = lower if lower is not None else upper
    if neighbour is None:
        return set()
    return {candidate for candidate in candidates if int(candidate) == neighbour}


def _reconcile_toc_spacing_from_ocr(
    lines: list[_PdfLine],
    ocr_markdown: str | None,
) -> list[_PdfLine]:
    """Restore only missing word boundaries corroborated by the local OCR layer."""

    ocr_lines = _visible_ocr_lines(ocr_markdown or "")
    if not ocr_lines:
        return lines

    def compact_with_boundaries(text: str) -> tuple[str, set[int]]:
        compact: list[str] = []
        boundaries: set[int] = set()
        pending_space = False
        for character in text.strip():
            if character.isspace():
                pending_space = bool(compact)
                continue
            if pending_space:
                boundaries.add(len(compact))
            compact.append(character)
            pending_space = False
        return "".join(compact), boundaries

    reconciled: list[_PdfLine] = []
    for line in lines:
        match = _best_matching_text_line(line.text, ocr_lines)
        if match is None or match[0] < 0.86:
            reconciled.append(line)
            continue
        candidate = _aligned_visual_candidate(line.text, match[1])
        native_compact, native_boundaries = compact_with_boundaries(line.text)
        ocr_compact, ocr_boundaries = compact_with_boundaries(candidate)
        if (
            len(native_compact) != len(ocr_compact)
            or _levenshtein_distance(native_compact.casefold(), ocr_compact.casefold()) > 2
        ):
            reconciled.append(line)
            continue
        added_boundaries = {
            position
            for position in ocr_boundaries - native_boundaries
            if 0 < position < len(native_compact)
            and native_compact[position - 1].isalpha()
            and native_compact[position].isalpha()
        }
        if not added_boundaries:
            reconciled.append(line)
            continue
        all_boundaries = native_boundaries | added_boundaries
        text = "".join(
            f" {character}" if index in all_boundaries else character
            for index, character in enumerate(native_compact)
        )
        reconciled.append(replace(line, text=text))
    return reconciled


def _toc_lines_share_row(entry: _PdfLine, number: _PdfLine) -> bool:
    entry_height = max(0.1, entry.bottom - entry.top)
    number_height = max(0.1, number.bottom - number.top)
    shorter_height = min(entry_height, number_height)
    overlap = min(entry.bottom, number.bottom) - max(entry.top, number.top)
    entry_center = (entry.top + entry.bottom) / 2
    number_center = (number.top + number.bottom) / 2
    return overlap >= shorter_height * 0.55 or abs(entry_center - number_center) <= max(
        1.5,
        shorter_height * 0.35,
    )


def _is_bold_font(font_name: str) -> bool:
    normalized = font_name.casefold()
    return any(marker in normalized for marker in ("bold", "black", "demi", "semibold"))


def _reconcile_suspicious_numbers_from_ocr(
    pages: list[_PdfPage],
    ocr_pages: dict[int, str],
) -> tuple[list[_PdfPage], int]:
    """Repair a broken numeric token only when its aligned OCR line selects one value."""

    replacements: dict[tuple[int, float, float, str], str] = {}
    for page in pages:
        ocr_markdown = ocr_pages.get(page.number)
        if not ocr_markdown:
            continue
        ocr_lines = _visible_ocr_lines(ocr_markdown)
        for line in page.lines:
            if line.rotated:
                continue
            suspicious_matches = _suspicious_numeric_glyph_matches(line.text)
            if not suspicious_matches:
                continue
            suspicious_spans = {match.span() for match in suspicious_matches}
            match = _best_matching_text_line(line.text, ocr_lines)
            if match is None or match[0] < 0.72:
                continue
            confirmed_numbers = set(re.findall(r"(?<!\d)\d{1,4}(?!\d)", match[1]))
            if not confirmed_numbers:
                continue

            def reconcile_token(
                candidate: re.Match[str],
                confirmed_numbers: set[str] = confirmed_numbers,
                suspicious_spans: set[tuple[int, int]] = suspicious_spans,
            ) -> str:
                if candidate.span() not in suspicious_spans:
                    return candidate.group(0)
                values = _numeric_glyph_candidates(candidate.group(0)) & confirmed_numbers
                return next(iter(values)) if len(values) == 1 else candidate.group(0)

            proposed = _SUSPICIOUS_NUMERIC_GLYPH_PATTERN.sub(reconcile_token, line.text)
            if proposed != line.text:
                replacements[_visual_line_key(line)] = proposed
    if not replacements:
        return pages, 0
    reconciled = [_apply_page_text_replacements(page, replacements) for page in pages]
    LOGGER.info("pdf_numeric_glyph_consensus_completed accepted=%d", len(replacements))
    return reconciled, len(replacements)


def _apply_page_text_replacements(
    page: _PdfPage,
    replacements: dict[tuple[int, float, float, str], str],
) -> _PdfPage:
    """Apply accepted visual readings to both flow lines and their structured table cells."""

    changed_lines = tuple(
        replace(line, text=replacements[_visual_line_key(line)])
        if _visual_line_key(line) in replacements
        else line
        for line in page.lines
    )
    if not page.tables:
        return replace(page, lines=changed_lines) if changed_lines != page.lines else page

    changed_tables: list[_PdfTable] = []
    for table in page.tables:
        table_replacements = tuple(
            (line.text, replacements[_visual_line_key(line)])
            for line in page.lines
            if _visual_line_key(line) in replacements and _line_inside_table(line, table)
        )
        if not table_replacements:
            changed_tables.append(table)
            continue
        rows = [list(row) for row in table.rows]
        for old, new in table_replacements:
            matches = [
                (row_index, column_index)
                for row_index, row in enumerate(rows)
                for column_index, cell in enumerate(row)
                if old in cell
            ]
            if len(matches) == 1:
                row_index, column_index = matches[0]
                if rows[row_index][column_index].count(old) == 1:
                    rows[row_index][column_index] = rows[row_index][column_index].replace(
                        old, new, 1
                    )
                    continue
            atom_replacement = _single_visual_atom_replacement(old, new)
            if atom_replacement is None:
                continue
            old_atom, new_atom = atom_replacement
            atom_matches = [
                (row_index, column_index, match.start(), match.end())
                for row_index, row in enumerate(rows)
                for column_index, cell in enumerate(row)
                for match in _VISUAL_ATOM_PATTERN.finditer(cell)
                if match.group(0) == old_atom
            ]
            if len(atom_matches) != 1:
                continue
            row_index, column_index, start, end = atom_matches[0]
            cell = rows[row_index][column_index]
            rows[row_index][column_index] = cell[:start] + new_atom + cell[end:]
        updated_rows = tuple(tuple(row) for row in rows)
        changed_tables.append(
            replace(table, rows=updated_rows) if updated_rows != table.rows else table
        )
    tables = tuple(changed_tables)
    if changed_lines == page.lines and tables == page.tables:
        return page
    return replace(page, lines=changed_lines, tables=tables)


def _single_visual_atom_replacement(old: str, new: str) -> tuple[str, str] | None:
    old_atoms = _visual_atoms(old)
    new_atoms = _visual_atoms(new)
    if not old_atoms or len(old_atoms) != len(new_atoms):
        return None
    changed = [
        (old_atom, new_atom)
        for old_atom, new_atom in zip(old_atoms, new_atoms, strict=True)
        if old_atom != new_atom
    ]
    return changed[0] if len(changed) == 1 else None


def _best_matching_text_line(
    source: str,
    candidates: tuple[str, ...],
) -> tuple[float, str] | None:
    ranked = sorted(
        (
            (
                SequenceMatcher(
                    None,
                    source.casefold(),
                    candidate.casefold(),
                    autojunk=False,
                ).ratio(),
                candidate,
            )
            for candidate in candidates
        ),
        reverse=True,
    )
    return ranked[0] if ranked else None


def _aligned_visual_candidate(native: str, candidate: str) -> str:
    native_atoms = _visual_atoms(native)
    candidate_atoms = _visual_atoms(candidate)
    if (
        len(candidate_atoms) == len(native_atoms) + 1
        and candidate_atoms[-1].isdigit()
        and not any(atom.isdigit() for atom in native_atoms)
    ):
        return re.sub(r"\s+\d{1,6}\s*$", "", candidate).strip()
    return candidate


def _visible_ocr_lines(markdown: str) -> tuple[str, ...]:
    lines: list[str] = []
    for raw_line in markdown.splitlines():
        stripped = raw_line.strip()
        if not stripped or re.fullmatch(r"\|?[\s:|-]+\|?", stripped):
            continue
        from_table = "|" in stripped and stripped.startswith("|")
        if from_table:
            stripped = " ".join(
                cell.strip() for cell in stripped.strip("|").split("|") if cell.strip()
            )
        stripped = re.sub(r"^[#>*+\-]+\s*", "", stripped)
        if not from_table:
            stripped = re.sub(r"^\d+[.)]\s+", "", stripped)
        stripped = re.sub(r"[*_`]+", "", stripped)
        stripped = re.sub(r"!?\[([^\]]*)\]\((?:<[^>]+>|[^)]+)\)", r"\1", stripped)
        stripped = re.sub(r"<[^>]+>", " ", stripped)
        stripped = " ".join(stripped.split())
        if stripped:
            lines.append(stripped)
    return tuple(dict.fromkeys(lines))


def _visual_atoms(text: str) -> tuple[str, ...]:
    return tuple(_VISUAL_ATOM_PATTERN.findall(unicodedata.normalize("NFC", text)))


def _visual_line_key(line: _PdfLine) -> tuple[int, float, float, str]:
    return (line.page_number, line.top, line.x0, line.text)


def _levenshtein_distance(left: str, right: str) -> int:
    if left == right:
        return 0
    if len(left) < len(right):
        left, right = right, left
    previous = list(range(len(right) + 1))
    for left_index, left_character in enumerate(left, start=1):
        current = [left_index]
        for right_index, right_character in enumerate(right, start=1):
            current.append(
                min(
                    current[-1] + 1,
                    previous[right_index] + 1,
                    previous[right_index - 1] + (left_character != right_character),
                )
            )
        previous = current
    return previous[-1]


def _line_inside_table(line: _PdfLine, table: _PdfTable) -> bool:
    x0, top, x1, bottom = table.bbox
    center_x = (line.x0 + line.x1) / 2
    center_y = (line.top + line.bottom) / 2
    return x0 <= center_x <= x1 and top <= center_y <= bottom


def _suspicious_numeric_glyph_matches(text: str) -> tuple[re.Match[str], ...]:
    """Return numeric-looking damage outside ordinary identifiers and literal URLs."""

    url_spans = tuple(match.span() for match in _LITERAL_URL_PATTERN.finditer(text))
    matches: list[re.Match[str]] = []
    for match in _SUSPICIOUS_NUMERIC_GLYPH_PATTERN.finditer(text):
        if any(start <= match.start() and match.end() <= end for start, end in url_spans):
            continue
        token = match.group(0)
        if re.fullmatch(r"\$\d+", token):
            continue
        if re.search(r"\d[$^]|[$^]\d", token) or _looks_like_broken_numeric_token(token):
            matches.append(match)
    return tuple(matches)


def _looks_like_broken_numeric_token(token: str) -> bool:
    """Require every letter in a mixed token to be a known number-shaped glyph."""

    if not (
        any(character.isdigit() for character in token)
        and any(character.isalpha() for character in token)
    ):
        return False
    letters = tuple(character for character in token if character.isalpha())
    if not letters or any(character not in _NUMERIC_GLYPH_EXPANSIONS for character in letters):
        return False
    candidates = _numeric_glyph_candidates(token)
    if token[0].isalpha() and token[-1].isalpha():
        return any(len(candidate) == len(token) for candidate in candidates)
    return bool(candidates)


def _hard_hyphen_wraps_word(previous: str, current: str) -> bool:
    if not current[:1].isalpha():
        return False
    if current[:1].islower():
        return True
    previous_word = previous.rstrip("-").rsplit(maxsplit=1)[-1]
    current_word = current.split(maxsplit=1)[0]
    return previous_word.isupper() and current_word.isupper()


def _repair_ocr_spacing_from_native(page: _PdfPage, markdown: str) -> str:
    """Restore spacing only when an OCR line has exactly the native line's characters."""

    native_words = {
        word.casefold()
        for line in page.lines
        if not line.rotated
        for word in re.findall(r"[^\W\d_]{5,}", line.text, re.UNICODE)
    }
    candidates: defaultdict[str, list[_PdfLine]] = defaultdict(list)
    for native_line in page.lines:
        key = _spacing_insensitive_line_key(native_line.text)
        if not native_line.rotated and len(key) >= 12:
            candidates[key].append(native_line)

    lines = markdown.splitlines()
    fence: str | None = None
    for index, line in enumerate(lines):
        stripped = line.strip()
        marker = (
            "```" if stripped.startswith("```") else "~~~" if stripped.startswith("~~~") else None
        )
        if marker is not None:
            if fence is None:
                fence = marker
            elif marker == fence:
                fence = None
            continue
        if (
            fence is not None
            or not stripped
            or stripped.startswith(("#", ">", "|", "![", "- ", "+ ", "* "))
        ):
            continue
        compact_line = _spacing_insensitive_text_with_offsets(stripped)
        if compact_line is not None:
            compact, offsets = compact_line
            replacements: list[tuple[int, int, str]] = []
            for key, native in candidates.items():
                if len(native) != 1:
                    continue
                compact_start = compact.find(key)
                if compact_start < 0 or compact.find(key, compact_start + 1) >= 0:
                    continue
                compact_end = compact_start + len(key) - 1
                start = offsets[compact_start]
                end = offsets[compact_end] + 1
                source_segment = stripped[start:end]
                native_text = native[0].text.strip()
                if len(re.findall(r"\s", source_segment)) < len(re.findall(r"\s", native_text)) + 2:
                    continue
                if any(
                    start < previous_end and end > previous_start
                    for previous_start, previous_end, _ in replacements
                ):
                    continue
                replacements.append((start, end, native_text))
            for start, end, replacement in sorted(replacements, reverse=True):
                stripped = f"{stripped[:start]}{replacement}{stripped[end:]}"
            if replacements:
                leading = line[: len(line) - len(line.lstrip())]
                lines[index] = f"{leading}{stripped}"
        repaired_words = stripped
        for pattern in (_SPACED_WORD_PATTERN, _PARTIAL_SPACED_WORD_PATTERN):
            repaired_words = pattern.sub(
                lambda match: (
                    compact
                    if (compact := re.sub(r"\s+", "", match.group())).casefold() in native_words
                    else match.group()
                ),
                repaired_words,
            )
        if repaired_words != stripped:
            leading = line[: len(line) - len(line.lstrip())]
            lines[index] = f"{leading}{repaired_words}"
            stripped = repaired_words
        native = candidates.get(_spacing_insensitive_line_key(stripped), [])
        if len(native) != 1:
            continue
        native_text = native[0].text.strip()
        if len(re.findall(r"\s", stripped)) < len(re.findall(r"\s", native_text)) + 2:
            continue
        leading = line[: len(line) - len(line.lstrip())]
        lines[index] = f"{leading}{native_text}"
    return "\n".join(lines)


def _spacing_insensitive_text_with_offsets(text: str) -> tuple[str, tuple[int, ...]] | None:
    compact: list[str] = []
    offsets: list[int] = []
    for index, character in enumerate(text):
        if character.isspace():
            continue
        normalized = unicodedata.normalize("NFKC", character).casefold()
        if len(normalized) != 1:
            return None
        compact.append(normalized)
        offsets.append(index)
    return "".join(compact), tuple(offsets)


def _normalize_text(text: str) -> str:
    normalized = unicodedata.normalize("NFC", text).replace("\u00ad", "")
    normalized = _repair_suspicious_glyph_encoding(normalized)
    return _WHITESPACE_PATTERN.sub(" ", normalized).strip()


def _repair_suspicious_glyph_encoding(text: str) -> str:
    lowercase_accents = str.maketrans("aeiou", "áéíóú")
    uppercase_accents = str.maketrans("AEIOU", "ÁÉÍÓÚ")

    def accent_vowel(match: re.Match[str]) -> str:
        vowel = match.group(1)
        table = uppercase_accents if vowel.isupper() else lowercase_accents
        return vowel.translate(table)

    repaired = re.sub(r"([aeiouAEIOU])\$(?=[^\W\d_])", accent_vowel, text)

    def repair_word_final_vowel(match: re.Match[str]) -> str:
        word = match.group(1)
        if any(character in "áéíóúÁÉÍÓÚ" for character in word):
            return word
        vowel = word[-1]
        table = uppercase_accents if vowel.isupper() else lowercase_accents
        return f"{word[:-1]}{vowel.translate(table)}"

    repaired = re.sub(
        r"([^\W\d_]*[aeiouAEIOU])\$(?=\s|$|[.,;:!?])",
        repair_word_final_vowel,
        repaired,
    )
    repaired = re.sub(
        r"([bcdfghjklmnpqrstvwxyzBCDFGHJLMNPQRSTVWXYZ])\$\s+(?=[a-záéíóúñ])",
        r"\1",
        repaired,
    )
    repaired = re.sub(
        r"(?i)n\"(?=[a-záéíóú])",
        lambda match: "Ñ" if match.group().isupper() else "ñ",
        repaired,
    )
    # In the affected legacy encoding, ``K`` stands in for an acute accent before
    # a consonant (``PRAKCTICA`` -> ``PRÁCTICA``).  Treating it as a marker before
    # any uppercase letter corrupts ordinary English words such as ``MAKE`` and
    # ``TAKE``.  Ambiguous vowel-to-vowel cases stay native so OCR/visual review can
    # arbitrate them instead of silently deleting a real character.
    repaired = re.sub(
        r"([AEIOU])K(?=[BCDFGHJLMNPQRSTVWXYZÑ]|$)",
        accent_vowel,
        repaired,
    )
    # Some embedded fonts map a digit to ``$``.  Removing every remaining
    # dollar sign used to turn a native TOC label such as ``1$.`` into ``1.``
    # before OCR had a chance to arbitrate it.  Only discard a residue still
    # attached to a natural-language word; keep numeric/currency uses visible.
    return re.sub(r"(?<=[^\W\d_])\$", "", repaired)


def _is_uppercase_text(text: str) -> bool:
    letters = "".join(character for character in text if character.isalpha())
    return bool(letters) and letters == letters.upper()


def _heading_letter_count(text: str) -> int:
    return sum(character.isalpha() for character in text)
