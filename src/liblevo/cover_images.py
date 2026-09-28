"""Bounded validation of newly selected cover images, preserving their exact bytes."""

from __future__ import annotations

import re
import warnings
from io import BytesIO
from pathlib import Path

from defusedxml import ElementTree

COVER_MEDIA_TYPES = {
    ".gif": "image/gif",
    ".jpeg": "image/jpeg",
    ".jpg": "image/jpeg",
    ".png": "image/png",
    ".svg": "image/svg+xml",
    ".webp": "image/webp",
}
MAX_COVER_BYTES = 20 * 1024 * 1024
_MAX_PIXELS = 16_000_000
_SVG_ELEMENTS = frozenset(
    "svg g defs title desc rect circle ellipse line polyline polygon path text tspan "
    "linearGradient radialGradient stop clipPath mask pattern".split()
)


def read_cover_image(path: Path) -> bytes:
    """Bound the read itself, including files replaced or grown after stat()."""
    if path.stat().st_size > MAX_COVER_BYTES:
        raise ValueError("La imagen de portada supera los 20 MB.")
    with path.open("rb") as stream:
        payload = stream.read(MAX_COVER_BYTES + 1)
    validate_cover_image(path.name, payload)
    return payload


def validate_cover_image(filename: str, payload: bytes) -> str:
    """Reject corrupt, mismatched, oversized or active images before storing them."""
    suffix = Path(filename).suffix.casefold()
    media_type = COVER_MEDIA_TYPES.get(suffix)
    if media_type is None:
        raise ValueError("La portada debe ser PNG, JPG, WEBP, GIF o SVG.")
    if not payload or len(payload) > MAX_COVER_BYTES:
        raise ValueError("La imagen de portada está vacía o supera los 20 MB.")
    try:
        if suffix == ".svg":
            _validate_svg(payload)
        else:
            from PIL import Image

            expected = "JPEG" if suffix in {".jpg", ".jpeg"} else suffix[1:].upper()
            with warnings.catch_warnings():
                warnings.simplefilter("error", Image.DecompressionBombWarning)
                with Image.open(BytesIO(payload)) as picture:
                    if picture.format != expected or picture.width * picture.height > _MAX_PIXELS:
                        raise ValueError("Formato incorrecto o imagen demasiado grande.")
                    if getattr(picture, "n_frames", 1) != 1:
                        raise ValueError("La portada debe ser una imagen estática.")
                    picture.verify()
                with Image.open(BytesIO(payload)) as picture:
                    picture.load()
    except Exception as exc:
        raise ValueError(
            "La portada no es una imagen válida y estática, es demasiado grande "
            "o contiene recursos externos. Elige otra imagen."
        ) from exc
    return media_type


def _validate_svg(payload: bytes) -> None:
    root = ElementTree.fromstring(payload, forbid_dtd=True)
    namespace = "{http://www.w3.org/2000/svg}"
    if root.tag != f"{namespace}svg":
        raise ValueError("SVG inválido.")
    for index, element in enumerate(root.iter()):
        if index >= 10_000 or element.tag not in {namespace + name for name in _SVG_ELEMENTS}:
            raise ValueError("SVG no estático o demasiado complejo.")
        for key, value in element.attrib.items():
            name = key.rsplit("}", 1)[-1].casefold()
            # No scripts, CSS escapes/imports, external references or recursive <use>.
            if name.startswith("on") or name == "base" or "\\" in value or "@" in value:
                raise ValueError("SVG activo.")
            if name == "href" and not value.startswith("#"):
                raise ValueError("Recurso externo en SVG.")
            for reference in re.findall(r"url\s*\((.*?)\)", value, re.IGNORECASE):
                if not reference.strip(" '\"").startswith("#"):
                    raise ValueError("Recurso externo en SVG.")
