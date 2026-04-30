#!/usr/bin/env python3
"""
Vending machine image analyzer.
Reads image bytes from stdin, outputs JSON analysis to stdout.

Output format:
  {"slots": [{"name": str|null, "price_yen": int|null, "temperature": str}]}
"""

import sys
import re
import json

try:
    import numpy as np
    import cv2
    import pytesseract
    from PIL import Image, ImageFilter
    import io
except ImportError as e:
    json.dump({"error": f"Missing dependency: {e}", "slots": []}, sys.stdout)
    sys.exit(0)


TEMPERATURE_UNKNOWN = "unknown"
TEMPERATURE_HOT = "hot"
TEMPERATURE_COLD = "cold"
TEMPERATURE_WARM = "warm"

HOT_THRESHOLD = 0.12
COLD_THRESHOLD = 0.12
WARM_THRESHOLD = 0.08

SLOT_ROWS = 5
MAX_LONG_SIDE = 1600  # リサイズ上限 px


def resize_for_ocr(img: np.ndarray) -> np.ndarray:
    """長辺を MAX_LONG_SIDE 以下にリサイズ（拡大はしない）。"""
    h, w = img.shape[:2]
    long_side = max(h, w)
    if long_side <= MAX_LONG_SIDE:
        return img
    scale = MAX_LONG_SIDE / long_side
    new_w = int(w * scale)
    new_h = int(h * scale)
    return cv2.resize(img, (new_w, new_h), interpolation=cv2.INTER_AREA)


def preprocess_for_ocr(region_bgr: np.ndarray) -> Image.Image:
    """
    OCR前処理:
      1. グレースケール
      2. 2x拡大（小さい文字のため）
      3. CLAHE でコントラスト強調
      4. 軽いシャープネス
    """
    gray = cv2.cvtColor(region_bgr, cv2.COLOR_BGR2GRAY)
    # 2倍に拡大してから OCR（小さい文字対策）
    h, w = gray.shape
    gray = cv2.resize(gray, (w * 2, h * 2), interpolation=cv2.INTER_CUBIC)
    # CLAHE でコントラスト強調
    clahe = cv2.createCLAHE(clipLimit=2.0, tileGridSize=(8, 8))
    gray = clahe.apply(gray)
    pil = Image.fromarray(gray)
    pil = pil.filter(ImageFilter.SHARPEN)
    return pil


def classify_temperature(bgr_region: np.ndarray) -> str:
    """HSV色分析でホット/コールド/ウォームを判定。"""
    if bgr_region.size == 0:
        return TEMPERATURE_UNKNOWN

    hsv = cv2.cvtColor(bgr_region, cv2.COLOR_BGR2HSV)
    h = hsv[:, :, 0].astype(np.int32)
    s = hsv[:, :, 1]
    v = hsv[:, :, 2]

    bright = (s > 80) & (v > 80)
    total = int(bright.sum()) + 1

    red_mask = ((h <= 10) | (h >= 170)) & bright
    blue_mask = ((h >= 100) & (h <= 130)) & bright
    orange_mask = ((h > 10) & (h < 25)) & bright

    if red_mask.sum() / total > HOT_THRESHOLD:
        return TEMPERATURE_HOT
    if blue_mask.sum() / total > COLD_THRESHOLD:
        return TEMPERATURE_COLD
    if orange_mask.sum() / total > WARM_THRESHOLD:
        return TEMPERATURE_WARM
    return TEMPERATURE_UNKNOWN


def extract_price(text: str) -> int | None:
    """OCRテキストから価格を抽出。"""
    # ¥NNN / NNN円 / ¥NNN（全角・半角・円記号混在に対応）
    patterns = [
        r'[¥￥\\]\s*(\d{2,4})',
        r'(\d{2,4})\s*[円¥]',
        r'\b(\d{3})\b',   # 3桁の数字（価格らしいもの）
    ]
    for pattern in patterns:
        m = re.search(pattern, text)
        if m:
            val = int(m.group(1))
            if 50 <= val <= 2000:
                return val
    return None


def clean_name(text: str) -> str | None:
    """
    OCRテキストから意味のある商品名行を抽出。
    - 記号・空白だけの行を除外
    - 最も長い「意味のある」行を返す
    """
    lines = [ln.strip() for ln in text.splitlines() if ln.strip()]
    # 日本語文字・英数字を含む行を候補に
    candidates = [
        ln for ln in lines
        if re.search(r'[゠-ヿ぀-ゟ一-鿿a-zA-Z0-9]', ln)
        and len(re.sub(r'[\s\W]', '', ln)) >= 2  # 有効文字 2文字以上
    ]
    if not candidates:
        return None
    return max(candidates, key=len)


def ocr_region(region_bgr: np.ndarray) -> str:
    """前処理付きでOCRを実行。psm 11 = スパーステキスト（自販機ラベルに向く）。"""
    pil = preprocess_for_ocr(region_bgr)
    text = pytesseract.image_to_string(
        pil, lang="jpn+eng",
        config="--psm 11 --oem 1"
    )
    return text


def analyze(image_bytes: bytes) -> list[dict]:
    arr = np.frombuffer(image_bytes, dtype=np.uint8)
    img = cv2.imdecode(arr, cv2.IMREAD_COLOR)
    if img is None:
        pil = Image.open(io.BytesIO(image_bytes)).convert("RGB")
        img = cv2.cvtColor(np.array(pil), cv2.COLOR_RGB2BGR)

    # 長辺リサイズ（OCR精度向上 + 処理高速化）
    img = resize_for_ocr(img)
    h, w = img.shape[:2]
    slot_h = max(1, h // SLOT_ROWS)

    slots = []
    for i in range(SLOT_ROWS):
        y0 = i * slot_h
        y1 = min(y0 + slot_h, h)
        row = img[y0:y1, :, :]

        # 左 20% をランプ領域、右 80% をラベル領域とする
        lamp_w = max(1, int(w * 0.20))
        lamp_region = row[:, :lamp_w, :]
        label_region = row[:, lamp_w:, :]

        temperature = classify_temperature(lamp_region)
        ocr_text = ocr_region(label_region)

        name = clean_name(ocr_text)
        price = extract_price(ocr_text)

        if name is None and price is None and temperature == TEMPERATURE_UNKNOWN:
            continue

        slots.append({
            "name": name,
            "price_yen": price,
            "temperature": temperature,
        })

    return slots


def main() -> None:
    image_bytes = sys.stdin.buffer.read()
    if not image_bytes:
        json.dump({"slots": []}, sys.stdout, ensure_ascii=False)
        return

    try:
        slots = analyze(image_bytes)
        json.dump({"slots": slots}, sys.stdout, ensure_ascii=False)
    except Exception as e:
        json.dump({"error": str(e), "slots": []}, sys.stdout, ensure_ascii=False)


if __name__ == "__main__":
    main()
