#!/usr/bin/env python3
"""
analyze_image.py の出力仕様テスト。
依存ライブラリ（cv2, pytesseract）がなくても実行できるよう、
analyze_image.py をモジュールとしてインポートするのではなく
subprocess 経由でテストする。

実行:
  python3 scripts/test_analyze_image.py
"""

import json
import subprocess
import sys
import os
import struct
import zlib

SCRIPT = os.path.join(os.path.dirname(__file__), "analyze_image.py")


def make_minimal_png(width: int = 10, height: int = 10) -> bytes:
    """最小限の PNG バイト列を生成（PIL 不要）。"""
    def write_chunk(chunk_type: bytes, data: bytes) -> bytes:
        length = struct.pack(">I", len(data))
        crc = struct.pack(">I", zlib.crc32(chunk_type + data) & 0xFFFFFFFF)
        return length + chunk_type + data + crc

    # IHDR: 10x10, 8-bit RGB
    ihdr_data = struct.pack(">IIBBBBB", width, height, 8, 2, 0, 0, 0)
    ihdr = write_chunk(b"IHDR", ihdr_data)

    # IDAT: 各行をフィルタバイト 0x00 + RGB(0,0,255) で埋める（青 = cold に対応）
    raw_rows = b""
    for _ in range(height):
        raw_rows += b"\x00" + b"\x00\x00\xff" * width
    compressed = zlib.compress(raw_rows)
    idat = write_chunk(b"IDAT", compressed)

    iend = write_chunk(b"IEND", b"")
    signature = b"\x89PNG\r\n\x1a\n"
    return signature + ihdr + idat + iend


class DependencyMissingError(RuntimeError):
    pass


def run_analyzer(image_bytes: bytes) -> dict:
    """analyze_image.py を subprocess で実行して JSON を返す。"""
    result = subprocess.run(
        [sys.executable, SCRIPT],
        input=image_bytes,
        capture_output=True,
        timeout=60,
    )
    stderr = result.stderr.decode("utf-8", errors="replace")
    if result.returncode != 0:
        # 依存ライブラリ不足は別扱い（Docker 外での実行を考慮）
        if "No module named" in stderr or "ModuleNotFoundError" in stderr:
            raise DependencyMissingError(
                f"依存ライブラリが不足しています（Docker コンテナ内で実行してください）: {stderr[:200]}"
            )
        raise RuntimeError(f"analyzer exited {result.returncode}: {stderr[:500]}")
    return json.loads(result.stdout.decode("utf-8"))


def test_output_schema_no_name_key():
    """T-02: 各スロットの出力に 'name' キーが含まれないことを検証。

    要件: 外観・価格・冷温のみ出力する。商品名は不要。
    現状(未修正): analyze_image.py は name フィールドを出力しているため、
      依存ライブラリがある Docker 環境ではこのテストが FAIL する。
    修正後: clean_name() を削除し slots から name キーを除去すれば PASS する。

    ※ ホスト環境でライブラリ不足の場合、slots が空のため本チェックはスキップされる。
      Docker コンテナ内で実行することで T-02 の FAIL を確認できる。
    """
    image_bytes = make_minimal_png()
    output = run_analyzer(image_bytes)

    assert "slots" in output, f"出力に 'slots' キーがない: {output}"
    for i, slot in enumerate(output["slots"]):
        assert "name" not in slot, (
            f"T-02 FAIL: slot[{i}] に 'name' キーが含まれている。"
            f" 要件は価格・冷温のみ。analyze_image.py から clean_name() を削除すること。"
            f" slot={slot}"
        )


def test_output_schema_required_keys():
    """T-02: 各スロットが 'price_yen' と 'temperature' を持つことを検証。"""
    image_bytes = make_minimal_png()
    output = run_analyzer(image_bytes)

    for i, slot in enumerate(output["slots"]):
        assert "price_yen" in slot, f"slot[{i}] に 'price_yen' がない: {slot}"
        assert "temperature" in slot, f"slot[{i}] に 'temperature' がない: {slot}"


def test_temperature_values_are_valid():
    """T-02: temperature の値が DB CHECK 制約と一致することを検証。"""
    valid_temps = {"hot", "cold", "warm", "unknown"}
    image_bytes = make_minimal_png()
    output = run_analyzer(image_bytes)

    for i, slot in enumerate(output["slots"]):
        temp = slot.get("temperature", "")
        assert temp in valid_temps, (
            f"slot[{i}] の temperature='{temp}' は DB CHECK 制約に違反する。"
            f" 許可値: {valid_temps}"
        )


def test_empty_input_returns_empty_slots():
    """analyze_image.py は空バイト列入力で slots が空リストを返すこと。
    依存ライブラリが不足している場合も slots:[] が返ることを確認する。
    """
    output = run_analyzer(b"")
    assert output.get("slots") == [], f"空入力時の slots は [] であるべき: {output}"


def test_price_yen_is_int_or_null():
    """T-02: price_yen は int または null（文字列ではない）であることを検証。"""
    image_bytes = make_minimal_png()
    output = run_analyzer(image_bytes)

    for i, slot in enumerate(output["slots"]):
        price = slot.get("price_yen")
        assert price is None or isinstance(price, int), (
            f"slot[{i}] の price_yen='{price}' は int または null であるべき"
        )


def run_tests():
    tests = [
        test_output_schema_no_name_key,
        test_output_schema_required_keys,
        test_temperature_values_are_valid,
        test_empty_input_returns_empty_slots,
        test_price_yen_is_int_or_null,
    ]

    passed = 0
    failed = 0
    skipped = 0

    for test_fn in tests:
        name = test_fn.__name__
        try:
            test_fn()
            print(f"  PASS  {name}")
            passed += 1
        except AssertionError as e:
            print(f"  FAIL  {name}")
            print(f"        {e}")
            failed += 1
        except DependencyMissingError as e:
            print(f"  SKIP  {name}")
            print(f"        {e}")
            skipped += 1
        except Exception as e:
            print(f"  ERROR {name}")
            print(f"        {type(e).__name__}: {e}")
            failed += 1

    print()
    print(f"結果: {passed} 通過 / {failed} 失敗 / {skipped} スキップ")
    if skipped > 0:
        print("  ※ スキップは依存ライブラリ不足のため。Docker コンテナ内で再実行してください。")
    if failed > 0:
        sys.exit(1)


if __name__ == "__main__":
    run_tests()
