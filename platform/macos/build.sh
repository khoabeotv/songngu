#!/bin/zsh
# Đóng gói bộ gõ macOS: target/Songngu.app. `--install`: chép vào ~/Library/Input Methods và đăng ký nguồn nhập
# (bật thì người dùng tự làm trong System Settings > Keyboard > Text Input > Edit > + > Vietnamese > Songngu).
set -euo pipefail
cd "$(dirname "$0")/../.."
export PATH="$HOME/.cargo/bin:$PATH"

cargo build --release --lib --bin songngu
# Thư viện engine chạy cả Apple Silicon lẫn Intel nếu có target x86_64 (rustup target add x86_64-apple-darwin).
LIB=target/release/libsongngu.a
ARCHS=(-arch arm64)
if [[ "$(rustup target list --installed 2>/dev/null)" == *x86_64-apple-darwin* ]]; then
  cargo build --release --lib --target aarch64-apple-darwin --target x86_64-apple-darwin
  lipo -create target/aarch64-apple-darwin/release/libsongngu.a target/x86_64-apple-darwin/release/libsongngu.a \
    -output target/libsongngu-universal.a
  LIB=target/libsongngu-universal.a
  ARCHS=(-arch arm64 -arch x86_64)
fi
APP=target/Songngu.app
rm -rf $APP
mkdir -p $APP/Contents/MacOS $APP/Contents/Resources/data
cp platform/macos/Info.plist $APP/Contents/
cp data/vi_50k.txt data/vi_extra.txt data/en_words.txt data/en_dev.txt $APP/Contents/Resources/data/

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
  [[ "$(shasum -a 256 $MODEL | cut -d' ' -f1)" == $MODEL_SHA256 ]] || echo "dùng $MODEL cục bộ (khác bản release $MODEL_TAG)" >&2
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
