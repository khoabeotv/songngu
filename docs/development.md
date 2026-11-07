# Phát triển

## Engine

```sh
cargo test --release
cargo run --release -- type "truowfng merge test cos" [--model models/model.bin]
cargo run --release -- type "xin visa~ xin visa"        # ~ = lật từ vừa chốt
cargo run --release -- eval <văn bản> [--model ...]    # đo ở mức engine: từ sai, sửa-khi-Space, đổi giữa từ
cargo run --release -- eval-session <văn bản> [--model ...] [--blind]   # đo qua đúng lớp bộ gõ (thay chữ hoặc xoá lùi)
cargo run --release -- score <got> <want>               # chấm lại output đã dump
cargo run --release -- bench <văn bản> [--model ...]
cargo run --release -- keys <văn bản> > f.keys          # chuỗi phím cho platform/macos/bench.c
cargo run --release -- ambig <văn bản> --out f.tsv     # từ phân vân kèm ngữ cảnh (cho tools/eval/rerank_train.py)
tools/eval/eval.sh dev|test models/model.bin            # bảng từ sai của bộ dev hoặc test
```

`<văn bản>`: mỗi dòng một tin nhắn; tuỳ chọn người gõ giả lập (`--realistic`, `--typos R`, `--seed N`) xem `evaluation.md`.

Tham số chỉ dò trên `dev`; `test` chỉ chạy để báo số. Thay đổi làm dev kém đi thì bỏ.

## Dữ liệu và mô hình

Nguồn và license: `data.md`. Corpus (`corpus/`) và mô hình (`models/`) không commit: mô hình là file đính kèm của release `MODEL_TAG` trong `platform/macos/build.sh`, build tự tải khi thiếu. Dựng lại khi đổi corpus hoặc định dạng mô hình trong engine.

```sh
curl -L -o corpus/fineweb/vie_test.parquet \
  https://huggingface.co/datasets/HuggingFaceFW/fineweb-2/resolve/main/data/vie_Latn/test/000_00000.parquet
uv run --with pyarrow tools/data/extract_fineweb.py corpus/fineweb/vie_test.parquet        # 6M từ + dev/test
curl -L -o corpus/fineweb/vie_train_000.parquet \
  https://huggingface.co/datasets/HuggingFaceFW/fineweb-2/resolve/main/data/vie_Latn/train/000_00000.parquet
uv run --with pyarrow tools/data/extract_fineweb_train.py corpus/fineweb/vie_train_000.parquet corpus/fineweb/train_big.txt
# kỹ thuật: tài liệu dịch CC BY 4.0 + trang kỹ thuật của shard FineWeb khác
git clone --depth 1 https://github.com/reactjs/vi.react.dev corpus/tech/src/vi.react.dev
git clone --depth 1 https://github.com/vuejs-translations/docs-vi corpus/tech/src/docs-vi
git clone --depth 1 --filter=blob:none --sparse https://github.com/kubernetes/website corpus/tech/src/k8s \
  && git -C corpus/tech/src/k8s sparse-checkout set content/vi
python3 tools/data/extract_docs.py corpus/tech/src
curl -L -o corpus/fineweb/vie_train_001.parquet \
  https://huggingface.co/datasets/HuggingFaceFW/fineweb-2/resolve/main/data/vie_Latn/train/000_00001.parquet
uv run --with pyarrow tools/data/extract_fineweb_tech.py corpus/fineweb/vie_train_001.parquet
# chat: eval_raw/ và own_raw/ do tools/data/gen_chat.workflow.js sinh; chủ đề eval i -> i%4: 0 dev_a, 1 dev_b, 2-3 test
python3 tools/data/clean_chat.py corpus/chatgen/test.txt <eval_raw chủ đề 2,3>
python3 tools/data/clean_chat.py corpus/chatgen/dev_a.txt <eval_raw chủ đề 0> --exclude corpus/chatgen/test.txt
python3 tools/data/clean_chat.py corpus/chatgen/dev_b.txt <eval_raw chủ đề 1> --exclude corpus/chatgen/{test,dev_a}.txt
python3 tools/data/clean_chat.py corpus/chatgen/own.txt corpus/chatgen/own_raw/*.txt --exclude corpus/chatgen/{dev_a,dev_b,test}.txt
python3 tools/data/build_tech.py                          # -> corpus/tech/{train,dev,test}.txt, không trùng bộ đo khác
cargo run --release -- save-model models/model.bin \
  --ctx "corpus/tech/train.txt*2,corpus/fineweb/train.txt,corpus/fineweb/train_big.txt,corpus/chatgen/own.txt*3"
```

- Kỹ thuật ×1/×2/×3/×5 cho 354/351/351/353 từ sai trên dev, giữ ×2. Chat ×2, ×3, ×4 ngang nhau trên dev, giữ ×3.
- Chỉ dùng nguồn có license cho phép phân phối dữ liệu dẫn xuất (`data.md`); nguồn mới phải ghi vào `data.md` và `NOTICE`.
- Phát hành mô hình mới: `gh release create model-vN models/model.bin --latest=false --title "Model vN" --notes "CC BY-SA 4.0, xem NOTICE"` (`--latest=false` để link cài đặt `releases/latest` vẫn trỏ bản app), rồi đổi `MODEL_TAG` và `MODEL_SHA256` (`shasum -a 256 models/model.bin`) trong `build.sh` và commit. Máy khác xoá `models/model.bin` để build tải bản mới; file cục bộ khác checksum thì build vẫn dùng và báo.
- Corpus giữ cả câu có âm tiết ngoài danh sách tần suất đã thử, kém hơn trên dev chat.
- Sinh chat: `tools/data/gen_chat.workflow.js` chạy qua pi-subagents (Claude Haiku 4.5, tối đa 64 lượt mỗi lần chạy); `args.set` = `train` (`args.part` 0/1) hoặc `eval`, `args.batch` = lượt thứ mấy của chủ đề.

## Bộ gõ macOS

```sh
platform/macos/build.sh             # -> target/Songngu.app (arm64 + x86_64 nếu có target x86_64-apple-darwin), kèm cửa sổ cài đặt
# chụp trang N (0-3) của cửa sổ cài đặt ra PNG, sáng hoặc tối; không cần quyền Ghi màn hình, chạy được cả khi máy khoá
SONGNGU_PAGE=0 SONGNGU_APPEARANCE=dark SONGNGU_SCREENSHOT=/tmp/shots caffeinate -d -u \
  target/Songngu.app/Contents/Helpers/SongnguSettings.app/Contents/MacOS/SongnguSettings
platform/macos/build.sh --install   # chép vào ~/Library/Input Methods và đăng ký nguồn nhập
clang -Iplatform/macos platform/macos/smoke.c target/release/libsongngu.a -o target/smoke   # chạy thử C ABI
target/smoke target/Songngu.app/Contents/Resources "truowfng tas"
clang -O2 -Iplatform/macos platform/macos/bench.c target/release/libsongngu.a -o target/bench
target/bench target/Songngu.app/Contents/Resources f.keys [--paced 10 10] [--idle 5]
```

`build.sh` đóng `models/model.bin` vào app (chưa có thì tải từ release, không được thì dựng từ corpus); không có thì bộ gõ chạy không ngữ cảnh.

Phát hành bản build sẵn (cài bằng `install.sh` qua `curl`): tăng `version` trong `Cargo.toml`, commit và push, rồi `platform/macos/release.sh --publish` trên máy có chứng chỉ "Songngu Dev". Script chạy test, build, nén `Songngu-<phiên bản>.zip`, điền phiên bản và checksum vào `install.sh`, tạo release `v<phiên bản>` đánh dấu Latest. Bỏ `--publish` để chỉ đóng gói vào `target/dist/`; thử cài từ file đó bằng `SONGNGU_ZIP=target/dist/Songngu-<phiên bản>.zip bash target/dist/install.sh`. Thử tự cập nhật không cần GitHub: dựng thư mục cùng cấu trúc (`releases/latest/download/install.sh`, `releases/download/v<phiên bản>/Songngu-<phiên bản>.zip`) rồi `SONGNGU_UPDATE_BASE=file:///thư/mục SongnguSettings --update --force`. Cùng chứng chỉ thì người dùng cập nhật không phải cấp lại quyền Trợ năng.

Cửa sổ cài đặt (SwiftUI) cần SDK macOS 26 trở lên mới có giao diện Liquid Glass: `build.sh` dùng Xcode trong `/Applications` nếu có (hoặc `SONGNGU_DEVELOPER_DIR`), không có thì Command Line Tools và cửa sổ ra kiểu cũ. Command Line Tools 16 (Swift 6.0) có lỗi `redefinition of module 'SwiftBridging'`; sửa bằng `sudo mv /Library/Developer/CommandLineTools/usr/include/swift/module.modulemap{,.bak}`.

## Cấu trúc repo

| Thư mục | Nội dung |
|---|---|
| `src/` | Engine Rust (thư viện + CLI `songngu`) |
| `tests/` | Test hành vi engine (`decide.rs`) và lớp bộ gõ (`ime.rs`) |
| `data/` | Từ điển tần suất Việt/Anh đi kèm |
| `platform/macos/` | Bộ gõ InputMethodKit + đường chặn phím, biểu tượng, script build; `settings/`: cửa sổ cài đặt SwiftUI |
| `tools/data/` | Tải, lọc corpus, sinh chat, lọc từ tiếng Anh |
| `tools/eval/` | Bảng đo dev/test, thử nghiệm bộ xếp hạng lại |
| `docs/` | Kiến trúc, phát triển, đo đạc, dữ liệu |
