"""Apply bilingual review patches under the shared fidelity guards."""

from __future__ import annotations

import json
import logging
import re
from collections import Counter
from dataclasses import dataclass

from parsezen.ai_markdown_safety import EMAIL_ADDRESS_PATTERN
from parsezen.ai_markdown_safety import (
    _prepare_and_validate_response as _prepare_and_validate_response,
)
from parsezen.ai_markdown_safety import _validate_mode_output as _validate_mode_output
from parsezen.errors import ImprovementError
from parsezen.improvement_contracts import ImprovementMode
from parsezen.translation_quality import (
    TranslationQualityError,
    established_term_translation,
    established_terms_requiring_translation,
    natural_language_text,
    validate_translation_quality,
)

LOGGER = logging.getLogger("parsezen.improvement")

RAW_URL_PATTERN = re.compile(r"(?:https?://|mailto:)[^\s<>)\]]+")


MAX_TRANSLATION_REVIEW_PATCHES = 64


MAX_TRANSLATION_REVIEW_PATCH_CHARACTERS = 1_200


@dataclass(frozen=True, slots=True)
class _TranslationReviewPart:
    source: str
    translated: str
    segment_numbers: frozenset[int] = frozenset()


def _source_words_protected_from_increase(value: str) -> frozenset[str]:
    return frozenset(
        word.casefold()
        for word in re.findall(
            r"[^\W\d_]{6,}",
            natural_language_text(value),
            flags=re.UNICODE,
        )
    )


def _translation_word_counts(value: str) -> Counter[str]:
    return Counter(
        word.casefold()
        for word in re.findall(
            r"[^\W\d_]{5,}",
            natural_language_text(value),
            flags=re.UNICODE,
        )
    )


def _apply_translation_review_patches(
    part: _TranslationReviewPart,
    response: str,
    *,
    source_language: str | None,
    target_language: str,
    require_target_language: bool = True,
    allowed_patch_terms: frozenset[str] | None = None,
    allow_source_text_retranslation: bool = False,
) -> str:
    raw_patches = _translation_review_patch_records(response)
    protected_literal_spans = _translation_review_protected_literal_spans(part.translated)

    patches: list[tuple[int, int, str]] = []
    seen_originals: set[str] = set()
    for raw_patch in raw_patches:
        if not isinstance(raw_patch, dict) or "old" not in raw_patch or "new" not in raw_patch:
            continue
        old = raw_patch.get("old")
        new = raw_patch.get("new")
        if (
            not isinstance(old, str)
            or not isinstance(new, str)
            or not old
            or old == new
            or len(old) > MAX_TRANSLATION_REVIEW_PATCH_CHARACTERS
            or len(new) > MAX_TRANSLATION_REVIEW_PATCH_CHARACTERS
            or "\0" in old
            or "\0" in new
        ):
            continue
        if old in seen_originals:
            continue
        seen_originals.add(old)
        if allowed_patch_terms is not None:
            old_counts = _translation_word_counts(old)
            new_counts = _translation_word_counts(new)
            if not any(new_counts[term] < old_counts[term] for term in allowed_patch_terms):
                continue
        if part.translated.count(old) != 1:
            continue
        raw_start = part.translated.index(old)
        offset, minimized_old, minimized_new = _minimize_translation_review_patch(old, new)
        if minimized_old == minimized_new or (not minimized_old and not minimized_new):
            continue
        start = raw_start + offset
        end = start + len(minimized_old)
        if (
            any(
                start < literal_end and end > literal_start
                for literal_start, literal_end in protected_literal_spans
            )
            or EMAIL_ADDRESS_PATTERN.search(minimized_new)
            or RAW_URL_PATTERN.search(minimized_new)
        ):
            continue
        patches.append((start, end, minimized_new))

    patches.sort(key=lambda patch: patch[0])
    accepted: list[tuple[int, int, str]] = []
    guard_rejections: Counter[str] = Counter()
    for patch in patches:
        if accepted and accepted[-1][1] > patch[0]:
            continue
        trial_patches = (*accepted, patch)
        pieces: list[str] = []
        cursor = 0
        for start, end, replacement in trial_patches:
            pieces.extend((part.translated[cursor:start], replacement))
            cursor = end
        pieces.append(part.translated[cursor:])
        try:
            proposed = "".join(pieces)
            candidate = (
                proposed
                if allow_source_text_retranslation
                else _prepare_and_validate_response(
                    part.translated,
                    proposed,
                    None,
                    ImprovementMode.REVIEW_CONTENT,
                )
            )
            _validate_translation_review_candidate(
                part,
                candidate,
                source_language=source_language,
                target_language=target_language,
                require_target_language=require_target_language,
                allow_source_text_retranslation=allow_source_text_retranslation,
            )
        except ImprovementError as exc:
            guard_rejections[str(exc)] += 1
            continue
        accepted.append(patch)

    LOGGER.info(
        "translation_review_patches proposed=%d eligible=%d accepted=%d rejected=%d "
        "guard_reasons=%s",
        len(raw_patches),
        len(patches),
        len(accepted),
        len(raw_patches) - len(accepted),
        "|".join(f"{reason}:{count}" for reason, count in sorted(guard_rejections.items()))
        or "none",
    )
    if raw_patches and not accepted:
        raise ImprovementError("Ninguna corrección propuesta superó las guardas de fidelidad.")
    if not accepted:
        return part.translated

    pieces = []
    cursor = 0
    for start, end, replacement in accepted:
        pieces.extend((part.translated[cursor:start], replacement))
        cursor = end
    pieces.append(part.translated[cursor:])
    return "".join(pieces)


def _translation_review_protected_literal_spans(markdown: str) -> tuple[tuple[int, int], ...]:
    """Locate bare web values that a linguistic review must never edit."""

    return tuple(
        sorted(
            {
                *((match.start(), match.end()) for match in RAW_URL_PATTERN.finditer(markdown)),
                *(
                    (match.start(), match.end())
                    for match in EMAIL_ADDRESS_PATTERN.finditer(markdown)
                ),
            }
        )
    )


def _minimize_translation_review_patch(old: str, new: str) -> tuple[int, str, str]:
    """Keep shared context and surrounding layout outside a model-proposed replacement."""

    prefix = 0
    limit = min(len(old), len(new))
    while prefix < limit and old[prefix] == new[prefix]:
        prefix += 1
    old_core = old[prefix:]
    new_core = new[prefix:]

    suffix = 0
    limit = min(len(old_core), len(new_core))
    while suffix < limit and old_core[-(suffix + 1)] == new_core[-(suffix + 1)]:
        suffix += 1
    if suffix:
        old_core = old_core[:-suffix]
        new_core = new_core[:-suffix]

    old_leading = len(old_core) - len(old_core.lstrip())
    new_leading = len(new_core) - len(new_core.lstrip())
    if old_leading and new_leading:
        prefix += old_leading
        old_core = old_core[old_leading:]
        new_core = new_core[new_leading:]

    old_trailing = len(old_core) - len(old_core.rstrip())
    new_trailing = len(new_core) - len(new_core.rstrip())
    if old_trailing and new_trailing:
        old_core = old_core[:-old_trailing]
        new_core = new_core[:-new_trailing]

    inner_prefix = 0
    limit = min(len(old_core), len(new_core))
    while inner_prefix < limit and old_core[inner_prefix] == new_core[inner_prefix]:
        inner_prefix += 1
    prefix += inner_prefix
    old_core = old_core[inner_prefix:]
    new_core = new_core[inner_prefix:]

    inner_suffix = 0
    limit = min(len(old_core), len(new_core))
    while inner_suffix < limit and old_core[-(inner_suffix + 1)] == new_core[-(inner_suffix + 1)]:
        inner_suffix += 1
    if inner_suffix:
        old_core = old_core[:-inner_suffix]
        new_core = new_core[:-inner_suffix]
    return prefix, old_core, new_core


def _translation_review_patch_records(response: str) -> list[object]:
    stripped = response.strip()
    fence = re.fullmatch(r"```(?:json)?\s*\n([\s\S]*?)\n```", stripped, re.IGNORECASE)
    if fence is not None:
        stripped = fence.group(1).strip()
    elif re.match(r"^```(?:json)?\s*\n", stripped, re.IGNORECASE):
        stripped = re.sub(r"^```(?:json)?\s*\n", "", stripped, count=1, flags=re.IGNORECASE)

    try:
        parsed = json.loads(stripped)
    except json.JSONDecodeError:
        parsed = _recover_complete_json_array_items(stripped)
    if isinstance(parsed, dict) and set(parsed) == {"corrections"}:
        parsed = parsed["corrections"]
    if not isinstance(parsed, list) or len(parsed) > MAX_TRANSLATION_REVIEW_PATCHES:
        raise ImprovementError("El modelo no devolvió correcciones JSON válidas.")
    return parsed


def _recover_complete_json_array_items(value: str) -> list[object] | None:
    if not value.startswith("["):
        return None
    decoder = json.JSONDecoder()
    cursor = 1
    recovered: list[object] = []
    while cursor < len(value):
        while cursor < len(value) and value[cursor].isspace():
            cursor += 1
        if cursor >= len(value) or value[cursor] == "]":
            break
        try:
            item, cursor = decoder.raw_decode(value, cursor)
        except json.JSONDecodeError:
            break
        recovered.append(item)
        if len(recovered) > MAX_TRANSLATION_REVIEW_PATCHES:
            return None
        while cursor < len(value) and value[cursor].isspace():
            cursor += 1
        if cursor < len(value) and value[cursor] == ",":
            cursor += 1
            continue
        break
    return recovered or None


def _validate_translation_review_candidate(
    part: _TranslationReviewPart,
    candidate: str,
    *,
    source_language: str | None,
    target_language: str,
    require_target_language: bool = True,
    allow_source_text_retranslation: bool = False,
    allow_ocr_word_separation: bool = False,
) -> None:
    for pattern, label in (
        (RAW_URL_PATTERN, "las URL protegidas"),
        (EMAIL_ADDRESS_PATTERN, "los correos electrónicos protegidos"),
    ):
        current_values = Counter(match.group(0) for match in pattern.finditer(part.translated))
        candidate_values = Counter(match.group(0) for match in pattern.finditer(candidate))
        if current_values != candidate_values:
            raise ImprovementError(f"La revisión alteró {label}.")
    if allow_source_text_retranslation:
        try:
            validate_translation_quality(
                part.translated,
                candidate,
                source_language=target_language,
                target_language=None,
                preserve_paragraphs=True,
            )
        except TranslationQualityError as exc:
            raise ImprovementError(str(exc)) from exc
    else:
        _validate_mode_output(
            part.translated,
            candidate,
            ImprovementMode.REVIEW_CONTENT,
        )
    try:
        validate_translation_quality(
            part.source,
            candidate,
            source_language=source_language,
            target_language=target_language if require_target_language else None,
            preserve_paragraphs=True,
        )
    except TranslationQualityError as exc:
        raise ImprovementError(str(exc)) from exc
    source_words = _source_words_protected_from_increase(part.source)
    current_words = _translation_word_counts(part.translated)
    candidate_words = _translation_word_counts(candidate)
    separated_ocr_words = (
        {
            word
            for word in source_words
            if any(word != current_word and word in current_word for current_word in current_words)
        }
        if allow_ocr_word_separation
        else set()
    )
    if any(
        candidate_words[word] > current_words[word] and word not in separated_ocr_words
        for word in source_words
    ):
        raise ImprovementError("La revisión introdujo texto del idioma de origen.")
    if source_language is not None:
        for source_term in established_terms_requiring_translation(
            part.source,
            source_language,
            target_language,
        ):
            target_term = established_term_translation(
                source_term,
                source_language,
                target_language,
            )
            if target_term is None:
                continue
            target_pattern = rf"(?<!\w){re.escape(target_term)}(?!\w)"
            current_count = len(re.findall(target_pattern, part.translated, re.IGNORECASE))
            candidate_count = len(re.findall(target_pattern, candidate, re.IGNORECASE))
            if current_count > candidate_count:
                raise ImprovementError(
                    "La revisión alteró una equivalencia terminológica establecida."
                )
