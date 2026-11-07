#!/bin/zsh
# Đóng gói bộ gõ macOS: target/Songngu.app, kèm cửa sổ cài đặt (SwiftUI, platform/macos/settings/) ở
# Contents/Helpers/SongnguSettings.app.
# `--install`: chép vào ~/Library/Input Methods và đăng ký nguồn nhập (bật thì người dùng tự làm trong System Settings >
# Keyboard > Text Input > Edit > + > Vietnamese > Songngu).
set -euo pipefail
cd "$(dirname "$0")/../.."
export PATH="$HOME/.cargo/bin:$PATH"

cargo build --release --lib --bin songngu
# Chạy cả Apple Silicon lẫn Intel nếu có target x86_64 (rustup target add x86_64-apple-darwin).
LIB=target/release/libsongngu.a
ARCHS=(-arch arm64)
SWIFT_ARCHS=(arm64)
if [[ "$(rustup target list --installed 2>/dev/null)" == *x86_64-apple-darwin* ]]; then
  cargo build --release --lib --target aarch64-apple-darwin --target x86_64-apple-darwin
  lipo -create target/aarch64-apple-darwin/release/libsongngu.a target/x86_64-apple-darwin/release/libsongngu.a \
    -output target/libsongngu-universal.a
  LIB=target/libsongngu-universal.a
  ARCHS=(-arch arm64 -arch x86_64)
  SWIFT_ARCHS=(arm64 x86_64)
fi
APP=target/Songngu.app
SETTINGS_APP=$APP/Contents/Helpers/SongnguSettings.app
rm -rf $APP
mkdir -p $APP/Contents/MacOS $APP/Contents/Resources/data $SETTINGS_APP/Contents/MacOS
cp platform/macos/Info.plist $APP/Contents/
cp data/vi_50k.txt data/vi_extra.txt data/en_words.txt data/en_dev.txt $APP/Contents/Resources/data/
cp NOTICE LICENSE $APP/Contents/Resources/

# Cửa sổ cài đặt: giao diện macOS 26 (Liquid Glass) chỉ có khi build bằng SDK macOS 26 trở lên, nên ưu tiên Xcode trong
# /Applications (hoặc SONGNGU_DEVELOPER_DIR); không có thì Command Line Tools, cửa sổ ra kiểu cũ. Không đổi
# `xcode-select` của máy.
SWIFT_DEV=${SONGNGU_DEVELOPER_DIR:-}
[[ -z $SWIFT_DEV && -d /Applications/Xcode.app ]] && SWIFT_DEV=/Applications/Xcode.app/Contents/Developer
SWIFT_DEV=${SWIFT_DEV:-$(xcode-select -p)}
echo "cửa sổ cài đặt: SDK macOS $(DEVELOPER_DIR=$SWIFT_DEV xcrun --sdk macosx --show-sdk-version)"
SWIFT_OUT=()
for a in $SWIFT_ARCHS; do
  DEVELOPER_DIR=$SWIFT_DEV xcrun swiftc -O -swift-version 5 -target $a-apple-macos13.0 -import-objc-header platform/macos/songngu.h \
    platform/macos/settings/*.swift $LIB -o target/SongnguSettings-$a
  SWIFT_OUT+=(target/SongnguSettings-$a)
done
lipo -create $SWIFT_OUT -output $SETTINGS_APP/Contents/MacOS/SongnguSettings
cp platform/macos/SettingsInfo.plist $SETTINGS_APP/Contents/Info.plist
# phiên bản hiện ở trang Trạng thái
COMMIT="$(git rev-parse --short HEAD 2>/dev/null || true)$(git diff --quiet HEAD 2>/dev/null || echo +)"
/usr/libexec/PlistBuddy -c "Add :SongnguCommit string $COMMIT" $SETTINGS_APP/Contents/Info.plist

# Mô hình ngữ cảnh: file đính kèm của release MODEL_TAG, không commit (mỗi bản dựng lại khác gần hết file cũ, lịch sử
# git sẽ nặng thêm ~7 MB mỗi lần). Thiếu thì tải (gh nếu có, repo private cần `gh auth login`), rồi mới dựng từ corpus/.
# Phát hành mô hình mới: docs/development.md.
MODEL=models/model.bin
MODEL_TAG=model-v1
MODEL_SHA256=0ebab0fcd467920e6eb0ba542e4ff60cb377d662846aeae98ab399e5143929f3
if [[ ! -f $MODEL ]]; then
  mkdir -p models
  rm -f $MODEL.part
  if ! { command -v gh >/dev/null && gh release download $MODEL_TAG -p model.bin -O $MODEL.part 2>/dev/null; }; then
    repo=$(git remote get-url origin 2>/dev/null | sed -E 's#^(git@github\.com:|https://github\.com/)##; s#\.git$##')
    curl -fsSL -o $MODEL.part "https://github.com/$repo/releases/download/$MODEL_TAG/model.bin" 2>/dev/null || rm -f $MODEL.part
  fi
  if [[ -f $MODEL.part && "$(shasum -a 256 $MODEL.part | cut -d' ' -f1)" == $MODEL_SHA256 ]]; then
    mv $MODEL.part $MODEL
  elif [[ -f $MODEL.part ]]; then
    echo "model.bin tải về sai checksum, bỏ qua" >&2
    rm -f $MODEL.part
  fi
fi
CTX="corpus/tech/train.txt*2,corpus/fineweb/train.txt"
[[ -f corpus/fineweb/train_big.txt ]] && CTX="$CTX,corpus/fineweb/train_big.txt"
[[ -f corpus/chatgen/own.txt ]] && CTX="$CTX,corpus/chatgen/own.txt*3"
if [[ ! -f $MODEL && -f corpus/tech/train.txt && -f corpus/fineweb/train.txt ]]; then
  target/release/songngu save-model $MODEL --ctx "$CTX"
fi
if [[ -f $MODEL ]]; then
  sum=$(shasum -a 256 $MODEL | cut -d' ' -f1)
  if [[ $sum == $MODEL_SHA256 ]]; then
    echo $MODEL_TAG > $APP/Contents/Resources/model-version.txt
  else
    echo "dùng $MODEL cục bộ (khác bản release $MODEL_TAG)" >&2
    echo "cục bộ ${sum:0:8}" > $APP/Contents/Resources/model-version.txt
  fi
  cp $MODEL $APP/Contents/Resources/model.bin
else
  echo "không có $MODEL: bộ gõ chạy không ngữ cảnh, đoán kém hơn" >&2
fi

clang -fobjc-arc -O2 -mmacosx-version-min=13.0 -framework Cocoa -framework CoreText -o target/mkicon platform/macos/mkicon.m
target/mkicon $APP/Contents/Resources/icon.pdf
clang -fobjc-arc -O2 -Wall $ARCHS -mmacosx-version-min=12.0 -Iplatform/macos -framework Cocoa -framework InputMethodKit -framework Carbon \
  -framework ApplicationServices platform/macos/main.m platform/macos/SongnguController.m platform/macos/KeyTap.m $LIB -o $APP/Contents/MacOS/Songngu
# Ký bằng chứng chỉ "Songngu Dev" nếu có trong keychain: quyền Trợ năng gắn với chứng chỉ nên còn nguyên qua các lần
# build. Không có thì ký ad-hoc: mỗi lần build phải xoá Songngu khỏi danh sách Trợ năng rồi cấp lại.
SIGN=-
security find-identity -p codesigning 2>/dev/null | grep -q '"Songngu Dev"' && SIGN="Songngu Dev"
# app lồng bên trong ký trước, rồi mới tới app ngoài
codesign --force --sign "$SIGN" $SETTINGS_APP
codesign --force --sign "$SIGN" $APP
echo "built $APP"

if [[ "${1:-}" == "--install" ]]; then
  DEST="$HOME/Library/Input Methods"
  mkdir -p "$DEST"
  pkill -x Songngu || true
  rm -rf "$DEST/Songngu.app"
  cp -R $APP "$DEST/"
  "$DEST/Songngu.app/Contents/MacOS/Songngu" --register
  echo "installed to $DEST/Songngu.app"
fi
