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
  say "Tải Songngu ${VERSION}…"
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

# Hỏi một lần (chưa có dòng auto_update trong settings.tsv). Không có terminal để hỏi thì để tắt.
CFG="$HOME/Library/Application Support/Songngu/settings.tsv"
if ! grep -q $'^option\tauto_update\t' "$CFG" 2>/dev/null; then
  ans=n
  if { exec 3</dev/tty; } 2>/dev/null; then
    printf 'Tự động cập nhật khi có bản mới? Chỉ cửa sổ cài đặt kết nối GitHub, bộ gõ không truy cập mạng. [Y/n] '
    read -r ans <&3 || ans=n
    exec 3<&-
    [[ -z "$ans" ]] && ans=y
  fi
  case "$ans" in [yY]*) v=1 ;; *) v=0 ;; esac
  mkdir -p "$(dirname "$CFG")"
  printf 'option\tauto_update\t%s\n' "$v" >> "$CFG"
  say "Tự động cập nhật: $([[ $v == 1 ]] && echo bật || echo tắt) (đổi ở Cài đặt Songngu > Trạng thái)."
fi

# Bộ gõ chạy nền bắt phím (danh sách trong conflicts.tsv của app vừa cài): chỉ cảnh báo, không tự tắt hay gỡ.
running_ids=$(lsappinfo list 2>/dev/null | sed -n 's/.*bundleID="\([^"]*\)".*/\1/p')
found=()
while IFS=$'\t' read -r id name proc; do
  [[ -z "$id" || "$id" == \#* ]] && continue
  if grep -qxF "$id" <<<"$running_ids" || pgrep -xq "$proc" 2>/dev/null; then found+=("$name"); fi
done < "$APP/Contents/Resources/conflicts.tsv"
if (( ${#found[@]} )); then
  say ""
  say "Cảnh báo: ${found[*]} đang chạy. Bộ gõ này bắt phím toàn hệ thống nên chạy cùng Songngu sẽ ra dấu đôi, mất hoặc"
  say "nhảy chữ. Cách tắt (Songngu không tự làm):"
  say "  1. Thoát nó: biểu tượng của nó trên thanh menu > Thoát (Quit)."
  say "  2. Không cho mở cùng máy: System Settings > General > Login Items & Extensions, và tuỳ chọn mở cùng máy trong"
  say "     cài đặt của nó."
  say "  3. Thu quyền: System Settings > Privacy & Security > Accessibility, tắt nó."
  say "Bộ gõ nằm trong Input Sources (Telex của macOS, XKeyIM...) thì không xung đột."
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
