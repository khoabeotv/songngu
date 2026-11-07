#!/bin/bash
# Cài Songngu bản build sẵn vào ~/Library/Input Methods (không cần sudo, Rust hay Xcode):
#   curl -fsSL https://github.com/khoabeotv/songngu/releases/latest/download/install.sh | bash
# Gỡ:
#   curl -fsSL https://github.com/khoabeotv/songngu/releases/latest/download/install.sh | bash -s -- --uninstall
# Tải bằng curl thì file không bị gắn cờ "tải từ Internet", nên Gatekeeper không chặn bản chưa notarize. Đừng tải
# Songngu-*.zip bằng trình duyệt: macOS sẽ chặn bộ gõ mà thường không hiện nút "Open Anyway".
# release.sh điền VERSION/SHA256; SONGNGU_ZIP=<file zip> để thử cài từ file trên máy.
set -euo pipefail

VERSION="@VERSION@"
SHA256="@SHA256@"
REPO="khoabeotv/songngu"
URL="https://github.com/$REPO/releases/download/v$VERSION/Songngu-$VERSION.zip"
DEST="$HOME/Library/Input Methods"
APP="$DEST/Songngu.app"

say() { printf '%s\n' "$*"; }
fail() { printf 'Lỗi: %s\n' "$*" >&2; exit 1; }

[[ "$(uname -s)" == Darwin ]] || fail "Songngu chỉ chạy trên macOS."
major=$(sw_vers -productVersion | cut -d. -f1)
(( major >= 13 )) || fail "Cần macOS 13 trở lên (máy đang chạy $(sw_vers -productVersion))."

if [[ "${1:-}" == "--uninstall" ]]; then
  pkill -x Songngu 2>/dev/null || true
  pkill -x SongnguSettings 2>/dev/null || true
  rm -rf "$APP"
  say "Đã gỡ Songngu. Còn hai việc tự làm nếu muốn:"
  say "  - System Settings > Keyboard > Text Input > Edit: bỏ Songngu khỏi danh sách nguồn nhập."
  say "  - Xoá dữ liệu đã học và cài đặt: rm -rf ~/Library/Application\\ Support/Songngu"
  exit 0
fi

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
if [[ -n "${SONGNGU_ZIP:-}" ]]; then
  cp "$SONGNGU_ZIP" "$tmp/Songngu.zip"
else
  say "Tải Songngu $VERSION…"
  curl -fL --progress-bar -o "$tmp/Songngu.zip" "$URL" || fail "không tải được $URL"
fi
echo "$SHA256  $tmp/Songngu.zip" | shasum -a 256 -c - >/dev/null 2>&1 || fail "file tải về sai checksum, thử lại sau."
ditto -x -k "$tmp/Songngu.zip" "$tmp/x"
codesign --verify --deep --strict "$tmp/x/Songngu.app" 2>/dev/null || fail "chữ ký của Songngu.app không hợp lệ."

# bản cũ đang chạy thì tắt; hệ thống tự mở bản mới ở phím gõ tiếp theo
pkill -x Songngu 2>/dev/null || true
pkill -x SongnguSettings 2>/dev/null || true
mkdir -p "$DEST"
rm -rf "$APP"
mv "$tmp/x/Songngu.app" "$DEST/"
xattr -dr com.apple.quarantine "$APP" 2>/dev/null || true
register=$("$APP/Contents/MacOS/Songngu" --register 2>&1 || true)
say "Đã cài Songngu $VERSION vào $APP"

others=$(pgrep -lf "XKey|EVKey|OpenKey|GoTiengViet|PHTV|Unikey" 2>/dev/null | grep -v "Input Methods/Songngu" || true)
if [[ -n "$others" ]]; then
  say ""
  say "Cảnh báo: đang có bộ gõ khác chạy, sẽ tranh phím với Songngu (Ctrl+Space không đổi được bộ gõ, app không nhận"
  say "bộ gõ). Thoát nó và gỡ khỏi System Settings > General > Login Items:"
  say "$others"
fi

say ""
if [[ "$register" == *"Chưa bật"* ]]; then
  say "Bước tiếp theo (macOS bắt người dùng tự làm):"
  say "  1. System Settings > Keyboard > Text Input > Edit > + > Vietnamese > Songngu > Add (đang mở sẵn)."
  say "  2. Chọn SN trên thanh menu (Ctrl+Space để đổi qua lại với ABC)."
  say "  3. Gõ trong terminal và vài app cần quyền Trợ năng: System Settings > Privacy & Security > Accessibility,"
  say "     bật Songngu (menu SN > Cấp quyền Trợ năng… nếu chưa có trong danh sách)."
  open "x-apple.systempreferences:com.apple.Keyboard-Settings.extension" 2>/dev/null || true
else
  say "Songngu đã có trong danh sách nguồn nhập. Chuyển sang ABC rồi quay lại SN để dùng bản mới."
fi
