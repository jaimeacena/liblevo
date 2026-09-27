"""Pure normalization and preservation rules for translated table cells."""

from __future__ import annotations

import html
import logging
import re
from collections import Counter

import parsezen.translation_quality as translation_quality_module
from parsezen.ai_markdown_safety import (
    _is_safe_translation_html_table as _is_safe_translation_html_table,
)
from parsezen.errors import ImprovementError
from parsezen.improvement_contracts import _TranslationContext
from parsezen.revision import split_markdown_blocks
from parsezen.translation_quality import (
    NUMBER_PATTERN,
    TITLE_LANGUAGE_HINTS,
    established_compact_label_translation,
    established_term_translation,
    is_probable_third_language_compact_value,
    is_reference_or_catalogue_text,
    natural_language_text,
    replace_established_compact_term_residues,
    source_language_word_residues,
)

LOGGER = logging.getLogger("parsezen.improvement")

MAX_ALIGNED_TRANSLATION_CONTEXT_CHARACTERS = 420


def _html_table_text_node_context(table: str, node: re.Match[str]) -> str:
    """Return the complete visible parent-cell text for a split HTML text node."""

    for cell in re.finditer(
        r"<(?P<tag>td|th)\b[^>]*>(?P<body>.*?)</(?P=tag)\s*>",
        table,
        re.IGNORECASE | re.DOTALL,
    ):
        if not (cell.start("body") <= node.start() and node.end() <= cell.end("body")):
            continue
        visible = html.unescape(re.sub(r"<[^>]+>", " ", cell.group("body")))
        return re.sub(r"\s+", " ", visible).strip()[:MAX_ALIGNED_TRANSLATION_CONTEXT_CHARACTERS]
    return ""


def _log_table_numeric_surface_change(stage: str, source: str, translated: str) -> None:
    """Expose count-only diagnostics for safe tables without logging document values."""

    source_numbers = Counter(NUMBER_PATTERN.findall(source))
    translated_numbers = Counter(NUMBER_PATTERN.findall(translated))
    if source_numbers == translated_numbers:
        return
    LOGGER.info(
        "translation_table_numeric_surface_changed stage=%s source=%d translated=%d "
        "missing=%d added=%d source_entities=%d translated_entities=%d",
        stage,
        sum(source_numbers.values()),
        sum(translated_numbers.values()),
        sum((source_numbers - translated_numbers).values()),
        sum((translated_numbers - source_numbers).values()),
        len(re.findall(r"&#(?:x[0-9a-f]+|\d+);", source, re.IGNORECASE)),
        len(re.findall(r"&#(?:x[0-9a-f]+|\d+);", translated, re.IGNORECASE)),
    )


def _strip_added_table_text_markup(source: str, translated: str) -> str:
    """Remove model-added wrappers from a plain table text node."""

    if any(character in source for character in "<>"):
        return translated
    candidate = translated.strip()
    candidate = re.sub(
        r"</?(?:span|em|strong|b|i|small|sup|sub|mark|code|p|div|br)"
        r"(?:\s+[^<>\r\n]{0,80})?/?>",
        "",
        candidate,
        flags=re.IGNORECASE,
    )

    def unwrap_angle_text(match: re.Match[str]) -> str:
        body = match.group("body")
        return body if sum(character.isalpha() for character in body) >= 2 else match.group(0)

    candidate = re.sub(r"<(?P<body>[^<>\r\n]+)>", unwrap_angle_text, candidate)
    # A source node without angle signs cannot legitimately gain them during
    # translation. Drop any malformed or unmatched remnants while retaining
    # every translated word; the caller still escapes the resulting text.
    return candidate.replace("<", "").replace(">", "").strip()


def _table_validation_failure_kind(error: ImprovementError) -> str:
    reason = str(error).casefold()
    categories = (
        ("alignment", ("alineación", "unidades", "celdas")),
        ("protected_value", ("valor protegido", "marcador")),
        ("number", ("números", "fechas", "romanos")),
        ("coverage", ("omitido", "duplicado", "añadido", "reescribe")),
        ("language", ("idioma solicitado", "texto de origen")),
        ("structure", ("estructura", "markdown", "html")),
    )
    for category, markers in categories:
        if any(marker in reason for marker in markers):
            return category
    return "validation"


def _translate_established_table_cell(
    source: str,
    context: _TranslationContext,
) -> str | None:
    """Resolve exact conventional titles before a model sees a generated TOC cell."""

    if context.source_language is None:
        return None
    match = re.fullmatch(
        r"(?P<prefix>(?:(?:\d{1,4}|\d[A-Za-z])[.):·-][ \t]+)?)"
        r"(?P<title>\S(?:.*\S)?)",
        source,
    )
    if match is None:
        return None
    source_title = match.group("title")
    if is_probable_third_language_compact_value(
        source_title,
        source_language=context.source_language,
        target_language=context.target_language,
    ):
        return source
    translated_title = established_compact_label_translation(
        source_title,
        context.source_language,
        context.target_language,
    )
    if translated_title is None and re.fullmatch(r"[IVXLCDM]{1,12}", source_title, re.IGNORECASE):
        return source
    if (
        translated_title is None
        and re.fullmatch(r"\([A-Za-zÀ-ÖØ-öø-ÿ'’\-]{3,48}\)", source_title)
        and not translation_quality_module._has_title_language_hint(
            source_title,
            context.source_language,
        )
    ):
        return source
    suffix = ""
    source_title_for_case = source_title
    if translated_title is None:
        suffix_match = re.fullmatch(
            r"(?P<title>\S(?:.*?\S)?)[ \t]+(?P<roman>[IVXLCDM]{1,8})",
            source_title,
        )
        if suffix_match is None:
            return None
        source_title_for_case = suffix_match.group("title")
        translated_title = established_term_translation(
            source_title_for_case,
            context.source_language,
            context.target_language,
        )
        if translated_title is None:
            return None
        suffix = f" {suffix_match.group('roman')}"
    if source_title_for_case.isupper():
        translated_title = translated_title.upper()
    if source_language_word_residues(
        source_title,
        translated_title,
        context.source_language,
    ):
        # A conventional-term replacement may improve only one word inside a longer free-form
        # cell.  That mixed result still needs the local model; it is not a deterministic answer.
        return None
    return f"{match.group('prefix')}{translated_title}{suffix}"


def _normalize_established_table_translation(
    source: str,
    translated: str,
    context: _TranslationContext,
) -> str:
    """Repair copied conventional labels without rewriting free-form table text."""

    if context.source_language is None:
        return translated
    normalized = replace_established_compact_term_residues(
        source,
        translated,
        context.source_language,
        context.target_language,
    )
    if translated.lstrip()[:1].isupper() and normalized.lstrip()[:1].islower():
        leading = len(normalized) - len(normalized.lstrip())
        normalized = (
            f"{normalized[:leading]}{normalized[leading].upper()}{normalized[leading + 1 :]}"
        )
    if (context.source_language, context.target_language) != ("en", "es"):
        return normalized
    source_part = re.search(
        r"(?<!\w)PART[ \t]+(?P<roman>[IVXLCDM]{1,8})(?!\w)",
        source,
        re.IGNORECASE,
    )
    translated_part = re.search(
        r"(?<!\w)(?:PART|PARTE)[ \t]+[IVXLCDM]{1,8}(?!\w)",
        normalized,
        re.IGNORECASE,
    )
    if source_part is None or translated_part is None:
        return normalized
    replacement = f"PARTE {source_part.group('roman').upper()}"
    if translated_part.group(0).islower():
        replacement = replacement.lower()
    return (
        f"{normalized[: translated_part.start()]}{replacement}{normalized[translated_part.end() :]}"
    )


def _ground_established_table_terms(
    source: str,
    context: _TranslationContext,
) -> str:
    """Pre-resolve known phrases inside a longer cell before local model translation."""

    if context.source_language is None:
        return source
    return replace_established_compact_term_residues(
        source,
        source,
        context.source_language,
        context.target_language,
    )


def _has_aligned_table_source_language_residue(
    source: str,
    translated: str,
    context: _TranslationContext,
) -> bool:
    """Validate each aligned table cell when reusing an otherwise valid checkpoint."""

    if not (
        _is_safe_translation_html_table(source) and _is_safe_translation_html_table(translated)
    ):
        return False
    source_nodes = list(re.finditer(r"(?<=>)[^<>]+(?=<)", source))
    translated_nodes = list(re.finditer(r"(?<=>)[^<>]+(?=<)", translated))
    if len(source_nodes) != len(translated_nodes):
        return False
    return any(
        _has_table_source_language_residue(
            html.unescape(source_node.group(0)).strip(),
            html.unescape(translated_node.group(0)).strip(),
            context.source_language,
            context.target_language,
        )
        for source_node, translated_node in zip(source_nodes, translated_nodes, strict=True)
        if sum(character.isalpha() for character in html.unescape(source_node.group(0))) >= 2
    )


def _normalize_aligned_table_translation(
    source: str,
    translated: str,
    context: _TranslationContext,
) -> str:
    """Upgrade a structurally aligned table, including a result loaded from cache."""

    if not (
        _is_safe_translation_html_table(source) and _is_safe_translation_html_table(translated)
    ):
        return translated
    source_nodes = list(re.finditer(r"(?<=>)[^<>]+(?=<)", source))
    translated_nodes = list(re.finditer(r"(?<=>)[^<>]+(?=<)", translated))
    if len(source_nodes) != len(translated_nodes):
        return translated
    replacements: list[tuple[int, int, str]] = []
    for source_node, translated_node in zip(source_nodes, translated_nodes, strict=True):
        source_value = html.unescape(source_node.group(0)).strip()
        translated_value = html.unescape(translated_node.group(0)).strip()
        if sum(character.isalpha() for character in source_value) < 2:
            continue
        normalized = _translate_established_table_cell(source_value, context)
        if normalized is None and _table_cell_translation_collapsed(
            source_value,
            translated_value,
        ):
            normalized = source_value
        if normalized is None:
            normalized = _normalize_established_table_translation(
                source_value,
                translated_value,
                context,
            )
        if normalized == translated_value:
            continue
        replacements.append(
            (
                translated_node.start(),
                translated_node.end(),
                _escaped_table_text_replacement(translated_node.group(0), normalized),
            )
        )
    for start, end, replacement in reversed(replacements):
        translated = f"{translated[:start]}{replacement}{translated[end:]}"
    return translated


def _table_cell_translation_collapsed(source: str, translated: str) -> bool:
    """Reject a substantive label collapsed to a numeral or tiny fragment."""

    source_visible = re.sub(r"\s+", " ", natural_language_text(source)).strip()
    translated_visible = re.sub(r"\s+", " ", natural_language_text(translated)).strip()
    source_letters = sum(character.isalpha() for character in source_visible)
    if source_letters < 12:
        return False
    if re.fullmatch(r"[IVXLCDM]{1,8}", translated_visible, re.IGNORECASE):
        return re.fullmatch(r"[IVXLCDM]{1,8}", source_visible, re.IGNORECASE) is None
    translated_letters = sum(character.isalpha() for character in translated_visible)
    return translated_letters < max(4, int(source_letters * 0.25))


def _escaped_table_text_replacement(source_node: str, translated_value: str) -> str:
    leading = source_node[: len(source_node) - len(source_node.lstrip())]
    trailing = source_node[len(source_node.rstrip()) :]
    return f"{leading}{html.escape(translated_value, quote=False)}{trailing}"


def _escaped_table_text_replacement_preserving_entities(
    source_node: str,
    translated_value: str,
) -> str:
    _protected_source, entities = _table_text_translation_value(source_node)
    return _restore_table_text_entities(
        _escaped_table_text_replacement(source_node, translated_value),
        entities,
    )


def _table_text_translation_value(source_node: str) -> tuple[str, tuple[tuple[str, str], ...]]:
    """Keep source HTML entities byte-exact while exposing surrounding text to translation."""

    entity_pattern = re.compile(r"&(?:#(?:x[0-9a-f]+|\d+)|[a-z][a-z0-9]+);", re.IGNORECASE)
    matches = tuple(entity_pattern.finditer(source_node))
    if not matches:
        return html.unescape(source_node), ()
    prefix = "PZTABLEENTITY"
    while prefix in source_node:
        prefix = f"Z{prefix}"
    protected = source_node
    entities: list[tuple[str, str]] = []
    for index, match in reversed(tuple(enumerate(matches))):
        token = f"`{prefix}{_alphabetic_marker_index(index)}XZQ`"
        entities.append((token, match.group(0)))
        protected = f"{protected[: match.start()]}{token}{protected[match.end() :]}"
    entities.reverse()
    return html.unescape(protected), tuple(entities)


def _alphabetic_marker_index(index: int) -> str:
    """Return a compact letter-only index that cannot be mistaken for document data."""

    letters: list[str] = []
    value = index
    while True:
        value, remainder = divmod(value, 26)
        letters.append(chr(ord("A") + remainder))
        if value == 0:
            break
        value -= 1
    return "".join(reversed(letters))


def _restore_table_text_entities(
    translated: str,
    entities: tuple[tuple[str, str], ...],
) -> str:
    restored = translated
    for token, entity in entities:
        if restored.count(token) == 1:
            restored = restored.replace(token, entity)
            continue
        serialized_value = html.escape(html.unescape(entity), quote=False)
        if (
            serialized_value
            and not serialized_value.isspace()
            and restored.count(serialized_value) == 1
        ):
            restored = restored.replace(serialized_value, entity)
            continue
        raise ImprovementError("La traducción tabular cambió una entidad HTML protegida.")
    return restored


def _aligned_table_response_values(response: str, expected_count: int) -> list[str]:
    blocks = [block.markdown.strip() for block in split_markdown_blocks(response)]
    if len(blocks) == expected_count:
        return blocks
    lines = [line.strip() for line in response.splitlines() if line.strip()]
    if len(lines) == expected_count:
        return lines
    return blocks


def _has_source_language_title_residue(
    source: str,
    translated: str,
    source_language: str | None,
    target_language: str | None = None,
) -> bool:
    if source_language is None:
        return False
    return bool(
        translation_quality_module._title_source_language_residues(
            source,
            translated,
            source_language,
            target_language,
        )
    )


def _has_table_source_language_residue(
    source: str,
    translated: str,
    source_language: str | None,
    target_language: str | None = None,
) -> bool:
    """Recognize compact residual labels inside cells, which are semantic title units."""

    if source_language is None:
        return False
    source_natural = re.sub(r"\s+", " ", natural_language_text(source)).strip()
    translated_natural = re.sub(r"\s+", " ", natural_language_text(translated)).strip()
    if (
        source_natural.casefold() == translated_natural.casefold()
        and source_natural[:1].islower()
        and sum(character.isalpha() for character in source_natural) >= 4
        and not is_reference_or_catalogue_text(source)
        and not is_probable_third_language_compact_value(
            source_natural,
            source_language=source_language,
            target_language=target_language or "",
        )
    ):
        # Lowercase index terms and short cell fragments are often too small for language
        # detection.  Exact copying is still a reliable signal that translation did not happen;
        # the guarded fallback may preserve genuine cross-language terms if no safe rewrite exists.
        return True
    if _has_source_language_title_residue(
        source,
        translated,
        source_language,
        target_language,
    ):
        return True
    if source_language_word_residues(
        natural_language_text(source),
        natural_language_text(translated),
        source_language,
    ):
        return True
    hints = TITLE_LANGUAGE_HINTS.get(source_language, frozenset())
    if not hints:
        return False
    source_words = {
        word.casefold()
        for word in re.findall(r"[^\W\d_]+", natural_language_text(source), re.UNICODE)
    }
    translated_words = {
        word.casefold()
        for word in re.findall(r"[^\W\d_]+", natural_language_text(translated), re.UNICODE)
    }
    return bool(source_words & translated_words & hints)
