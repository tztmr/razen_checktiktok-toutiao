import os
import plistlib
import re
import sys
import time
import zipfile

import requests


API_URL = "https://imdesktop.douyin.com/aweme/v1/web/user/profile/other/"
PLIST_SUFFIX = "/Library/Preferences/com.ss.iphone.ugc.Aweme.plist"
SECUID_PATTERN = re.compile(rb"MS4[\w\-_]+")


def find_aweme_plist(zf: zipfile.ZipFile) -> str | None:
    for inner in zf.namelist():
        if inner.lower().endswith(PLIST_SUFFIX.lower()):
            return inner
    return None


def collect_strings(value, output: list[str]) -> None:
    if isinstance(value, str):
        output.append(value)
        return
    if isinstance(value, bytes):
        try:
            output.append(value.decode("utf-8", "ignore"))
        except Exception:  # noqa: BLE001
            return
        return
    if isinstance(value, list):
        for item in value:
            collect_strings(item, output)
        return
    if isinstance(value, dict):
        for item in value.values():
            collect_strings(item, output)


def first_ms4(values: list[str]) -> str:
    for value in values:
        match = re.search(r"MS4[\w\-_]+", value)
        if match:
            return match.group(0)
    return ""


def extract_secuid_from_zip(zip_path: str) -> str:
    with zipfile.ZipFile(zip_path) as zf:
        plist_name = find_aweme_plist(zf)
        if not plist_name:
            raise ValueError("NO_AWEME_PLIST")
        data = zf.read(plist_name)
    root = plistlib.loads(data)

    matched_secuid = ""
    cache_value = root.get("AWEUserStorageCacheUserKey")
    if isinstance(cache_value, (bytes, bytearray)) and cache_value:
        try:
            nested = plistlib.loads(bytes(cache_value))
            values: list[str] = []
            if isinstance(nested, dict) and isinstance(nested.get("$objects"), list):
                collect_strings(nested.get("$objects"), values)
            else:
                collect_strings(nested, values)
            matched_secuid = first_ms4(values)
        except Exception:  # noqa: BLE001
            matched_secuid = ""
    elif isinstance(cache_value, str) and cache_value:
        try:
            nested = plistlib.loads(cache_value.encode("utf-8"))
            values = []
            if isinstance(nested, dict) and isinstance(nested.get("$objects"), list):
                collect_strings(nested.get("$objects"), values)
            else:
                collect_strings(nested, values)
            matched_secuid = first_ms4(values)
        except Exception:  # noqa: BLE001
            matched_secuid = ""

    guard_value = root.get("kTTAccountTicketGuardSecUserIdTsSignDic")
    guard_secuid = ""
    if isinstance(guard_value, dict) and guard_value:
        guard_secuid = next(iter(guard_value.keys()))

    secuid = matched_secuid or guard_secuid
    if not secuid:
        match = SECUID_PATTERN.search(data)
        if match:
            secuid = match.group(0).decode("ascii", "ignore")

    if not secuid:
        raise ValueError("NO_SECUID")
    return secuid


def resolve_unique_id(session: requests.Session, sec_uid: str) -> str:
    response = session.get(
        API_URL,
        params={
            "aid": "339757",
            "device_id": "7184690798967999755",
            "version_name": "1.0.0",
            "device_platform": "win32",
            "sec_user_id": sec_uid,
        },
        timeout=15,
    )
    response.raise_for_status()
    payload = response.json()
    unique_id = ((payload.get("user") or {}).get("unique_id") or "").strip()
    if payload.get("status_code") != 0 or not unique_id:
        raise ValueError(f"API_FAIL:{payload.get('status_msg') or payload.get('status_code')}")
    return unique_id


def main() -> int:
    if len(sys.argv) != 2:
        print("usage: extract_dy_uqid.py <directory>")
        return 2

    base = sys.argv[1]
    session = requests.Session()
    session.headers.update(
        {
            "User-Agent": (
                "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) "
                "AppleWebKit/537.36 (KHTML, like Gecko) Chrome/136.0.0.0 Safari/537.36"
            )
        }
    )

    results: list[tuple[str, str, str]] = []
    for name in sorted(os.listdir(base)):
        if not name.lower().endswith(".zip"):
            continue

        zip_path = os.path.join(base, name)
        try:
            sec_uid = extract_secuid_from_zip(zip_path)
            unique_id = resolve_unique_id(session, sec_uid)
            out_path = os.path.join(base, f"{name}_dy_uqid.txt")
            with open(out_path, "w", encoding="utf-8") as fh:
                fh.write(unique_id)
                fh.write("\n")
            results.append((name, "OK", unique_id))
        except Exception as exc:  # noqa: BLE001
            results.append((name, "FAIL", str(exc)))
        time.sleep(0.1)

    summary_path = os.path.join(base, "_dy_uqid_summary.txt")
    with open(summary_path, "w", encoding="utf-8") as fh:
        for name, status, value in results:
            fh.write(f"{name}\t{status}\t{value}\n")

    ok_count = sum(1 for _, status, _ in results if status == "OK")
    fail_count = len(results) - ok_count
    print(f"SUMMARY\t{summary_path}")
    print(f"TOTAL\t{len(results)}")
    print(f"OK\t{ok_count}")
    print(f"FAIL\t{fail_count}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
