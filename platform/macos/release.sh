#!/bin/zsh
# Đóng gói bản phát hành cho cách cài bằng một dòng lệnh (install.sh): target/dist/Songngu-<phiên bản>.zip + install.sh
# đã điền phiên bản và checksum. `--publish`: tạo GitHub Release v<phiên bản> ở commit HEAD (đã push) và đánh dấu là
# Latest, để link releases/latest/download/install.sh luôn trỏ bản mới nhất.
# Bản phát hành phải ký bằng chứng chỉ "Songngu Dev" của máy này: quyền Trợ năng của người dùng gắn với chứng chỉ, ký
# kiểu khác thì sau khi cập nhật họ phải cấp lại quyền. Phiên bản lấy từ Cargo.toml: tăng trước mỗi lần phát hành.
set -euo pipefail
cd "$(dirname "$0")/../.."
export PATH="$HOME/.cargo/bin:$PATH"

PUBLISH=0
[[ "${1:-}" == "--publish" ]] && PUBLISH=1
VERSION=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
TAG=v$VERSION

[[ -z "$(git status --porcelain)" ]] || { echo "còn thay đổi chưa commit: phát hành từ một commit sạch" >&2; exit 1; }
security find-identity -p codesigning 2>/dev/null | grep -q '"Songngu Dev"' || { echo "thiếu chứng chỉ Songngu Dev" >&2; exit 1; }
if (( PUBLISH )); then
  git fetch -q origin
  [[ "$(git rev-parse HEAD)" == "$(git rev-parse '@{u}')" ]] || { echo "HEAD chưa push lên origin" >&2; exit 1; }
  ! gh release view $TAG >/dev/null 2>&1 || { echo "$TAG đã có: tăng version trong Cargo.toml" >&2; exit 1; }
fi

cargo test --release -q >/dev/null 2>&1 || { echo "test hỏng: cargo test --release" >&2; exit 1; }
platform/macos/build.sh
APP=target/Songngu.app
codesign -dr - $APP 2>&1 | grep -q "certificate leaf" || { echo "app không ký bằng chứng chỉ" >&2; exit 1; }
[[ -f $APP/Contents/Resources/model.bin ]] || { echo "thiếu model.bin trong app" >&2; exit 1; }

DIST=target/dist
rm -rf $DIST
mkdir -p $DIST
ZIP=$DIST/Songngu-$VERSION.zip
# ditto giữ nguyên chữ ký và thuộc tính của bundle
ditto -c -k --keepParent $APP $ZIP
SUM=$(shasum -a 256 $ZIP | cut -d' ' -f1)
sed -e "s/@VERSION@/$VERSION/" -e "s/@SHA256@/$SUM/" platform/macos/install.sh > $DIST/install.sh
chmod +x $DIST/install.sh
echo "$ZIP ($(du -h $ZIP | cut -f1)), sha256 $SUM"

if (( PUBLISH )); then
  gh release create $TAG $ZIP $DIST/install.sh --target "$(git rev-parse HEAD)" --latest --title "Songngu $VERSION" --notes \
"Cài hoặc cập nhật (macOS 13 trở lên, Apple Silicon và Intel):

\`\`\`
curl -fsSL https://github.com/khoabeotv/songngu/releases/latest/download/install.sh | bash
\`\`\`

Đừng tải file zip bằng trình duyệt: bản chưa notarize, macOS sẽ chặn. SHA-256 của zip: \`$SUM\`."
fi
