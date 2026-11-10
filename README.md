# Songngu

Bộ gõ song ngữ Việt/Anh cho macOS, không có chế độ Anh/Việt: cứ gõ Telex, bộ gõ tự hiểu từ nào là tiếng Việt, từ nào là tiếng Anh ngay lúc gõ ("truowfng merge test cos" -> "trường merge test có"). Đoán sai thì bấm Backspace ngay sau từ đó để lật.

## Cài đặt

Cần macOS 13 trở lên (Apple Silicon hoặc Intel).

**Cài nhanh** (bản build sẵn, dùng cả để cập nhật): dán vào Terminal

```sh
curl -fsSL https://github.com/khoabeotv/songngu/releases/latest/download/install.sh | bash
```

Script chép Songngu vào `~/Library/Input Methods` (không cần sudo), hỏi có bật tự động cập nhật không, rồi mở cửa sổ cài đặt ở trang hướng dẫn các bước còn lại. Đừng tải file zip bằng trình duyệt: bản phát hành chưa notarize, macOS sẽ chặn bộ gõ.

Sau khi cài (macOS bắt người dùng tự làm):

1. Bật: System Settings > Keyboard > Text Input > Edit > **+** > Vietnamese > **Songngu**. Biểu tượng **SN** hiện trên thanh menu; Ctrl+Space đổi qua lại với ABC.
2. Gõ trong terminal (Ghostty, Terminal, iTerm2...) cần quyền Trợ năng: System Settings > Privacy & Security > Accessibility, bật Songngu (menu SN > "Cấp quyền Trợ năng…" nếu chưa có trong danh sách).

**Bộ gõ khác**: bộ gõ chạy nền bắt phím qua event tap và quyền Trợ năng (OpenKey, EVKey, GoTiengViet, PHTV, app XKey...) xung đột với Songngu: dấu đôi, mất hoặc nhảy chữ, Ctrl+Space không đổi được bộ gõ. Thoát app đó, tắt mở cùng máy (System Settings > General > Login Items & Extensions và cài đặt của chính nó), thu quyền ở Privacy & Security > Accessibility. `install.sh` và trang Trạng thái của cửa sổ cài đặt cảnh báo khi thấy chúng đang chạy (chỉ cảnh báo, không tự tắt). Bộ gõ nằm trong Input Sources (Telex có sẵn của macOS, XKeyIM...) không xung đột vì mỗi lúc chỉ một nguồn nhập được chọn; bỏ khỏi danh sách nếu muốn gọn.

**Gỡ**: cùng lệnh trên với `| bash -s -- --uninstall`, rồi bỏ Songngu khỏi danh sách Text Input. Dữ liệu đã học nằm ở `~/Library/Application Support/Songngu`.

**Build từ mã nguồn**: cần thêm Xcode Command Line Tools (`xcode-select --install`) và Rust (`curl https://sh.rustup.rs -sSf | sh`). Cửa sổ cài đặt có giao diện macOS 26 (Liquid Glass) khi máy có Xcode 26 trở lên; chỉ có Command Line Tools thì ra kiểu cũ.

```sh
platform/macos/build.sh --install
```

- Lần đầu build tự tải mô hình ngữ cảnh (22 MB) từ mục Releases về `models/model.bin` rồi đóng vào app. Không tải được thì bộ gõ vẫn chạy nhưng đoán kém hơn.
- Mỗi lần build lại, quyền Trợ năng mất hiệu lực, trừ khi có chứng chỉ ký mã tên **Songngu Dev**: Keychain Access > Certificate Assistant > Create a Certificate, Identity Type "Self Signed Root", Certificate Type "Code Signing". Có chứng chỉ thì `build.sh` tự ký bằng nó và quyền giữ nguyên qua các lần cập nhật.
- Cập nhật: `git pull` rồi chạy lại `build.sh --install`, chuyển sang ABC rồi quay lại Songngu.

## Sử dụng

**Gõ**: Telex, dấu đặt ở đâu trong âm tiết cũng được: `tieesng`, `tieengs`, `tienges` đều ra "tiếng"; `truowngf`, `truongwf`; `thaays`, `thayas`; `dduwowcj`, `duodwcj`; w gõ lỡ trước u cũng được (`chwua` ra "chưa"); hai phím gõ đảo làm từ không thành tiếng Việt thì bộ gõ thử đảo lại khi chốt từ (`nyaf` ra "này", `ddwang` ra "đăng"). Từ tiếng Anh gõ như bình thường. Gõ lặp phím dấu để huỷ dấu đó và giữ chữ cái. Kiểu viết chat không phải âm tiết tiếng Việt (`uwfk` → "ừk", `ddzai` → "đzai") thì gõ rồi bấm Ctrl+Esc; lật một lần là lần sau tự ra.

**Lật Anh/Việt** khi bộ gõ đoán sai:

| Phím | Lật gì |
|---|---|
| **Backspace** ngay sau từ (sau dấu cách hoặc dấu câu vừa gõ) | Từ vừa gõ, nếu cả hai phương án đều là từ có thật (`did`/đi, `theme`/thêm, `visa`/vía), trừ khi ngữ cảnh đã rất chắc ("of" trong "the cost of"); hoặc lần bộ gõ vừa tự sửa từ trước. Từ khác thì Backspace xoá như thường |
| **Ctrl+Esc** (hoặc Shift+Esc), mọi app | Từ đang gõ dở, từ vừa gõ, hoặc từ bất kỳ ngay trước con trỏ (click quay lại sau từ rồi bấm; không dùng được ở terminal và app không cho bộ gõ đọc chữ như Zalo) |

- Ví dụ: gõ `did ` ra "did ", bấm Backspace thành "đi ", gõ tiếp.
- Định xoá mà bộ gõ lại lật: bấm **Backspace lần nữa ngay**, chữ trả lại như cũ và dấu cách bị xoá. Giữ Backspace để xoá cả đoạn vẫn xoá đúng.
- Esc trơn luôn là của app (đóng hộp thoại, huỷ sửa tin nhắn). Muốn lật bằng Esc thay cho Backspace: menu SN > "Lật Anh/Việt bằng Esc thay cho Backspace". Khi đó Esc lật từ đang gõ và từ vừa gõ ở app thường, bấm Esc lần nữa ngay để trả lại.
- **Xoá cả từ rồi gõ lại đúng như cũ** cũng là lật: gõ `visa` ra "visa", xoá hết rồi gõ `visa` lần nữa thì ra "vía" (chỉ với từ mà cả hai phương án đều là từ có thật, như cách lật bằng Backspace).
- Mỗi lần lật, bộ gõ ghi nhớ lựa chọn theo ngữ cảnh: cùng cụm từ thì lần sau tự đúng; ở chỗ khác, mỗi lần lật làm cán cân nghiêng thêm (1 lần ×4, 2 lần ×9).

**Tự sửa từ trước**: bộ gõ có thể đổi lại từ vừa gõ khi thấy từ sau ("xin visa may" -> "xin vía may"). Không muốn thì bấm Backspace ngay lúc đó để hoàn tác.

**Menu SN** trên thanh menu:

| Mục | Ý nghĩa |
|---|---|
| Cài đặt Songngu… | Mở cửa sổ cài đặt (bên dưới) |
| Báo lỗi gõ… | Mở thẳng trang Báo lỗi của cửa sổ cài đặt |
| Bỏ qua app này | Songngu đứng ngoài app đang mở, phím đi thẳng vào app |
| Chặn phím ở app này | Sửa chữ bằng Backspace giả thay vì thay chữ trực tiếp. Tự bật cho terminal và app không cho thay chữ; bật tay khi một app gõ ra chữ lặp ("taá") |
| Cấp quyền Trợ năng… | Chỉ hiện khi chưa có quyền |

**Cửa sổ cài đặt**:

| Trang | Nội dung |
|---|---|
| Bắt đầu | Các bước macOS bắt người dùng tự làm (thêm nguồn nhập, chọn SN, quyền Trợ năng, tắt bộ gõ chạy nền khác), mỗi bước có nút mở đúng chỗ và tự đánh dấu khi xong. Cửa sổ mở trang này khi còn bước chưa xong |
| Gõ | Đặt dấu kiểu cũ (hòa, thủy); gõ VNI; `[ ]` gõ ơ ư; lật bằng Esc thay cho Backspace; chỉ chặn phím ở app trong danh sách (mặc định Songngu tự lo app không nhận bộ gõ, như XKey, đổi lại gõ chữ ở chỗ không có ô nhập như danh sách thư, Finder có thể sinh Backspace giả); ghi nhật ký gõ ra `typing-log.tsv` (để báo lỗi, không ghi trong ô mật khẩu) |
| Ứng dụng | App bỏ qua và app chặn phím: thêm, gỡ. App "tự phát hiện" là bộ gõ tự thêm, gỡ nếu app vẫn gõ đúng |
| Đã học | Các từ học được từ những lần lật và ngữ cảnh của chúng; thói quen gõ mũ/đ muộn (quen gõ "cana" cho "cân" thì bộ gõ dễ ra chữ Việt hơn); quên từng từ hoặc tất cả (file cũ giữ ở `memory-forgotten.tsv`) |
| Trạng thái | Bộ gõ có đang chạy, quyền Trợ năng, bộ gõ chạy nền khác đang xung đột; tự động cập nhật và kiểm tra cập nhật; phiên bản, mô hình, giấy phép |
| Báo lỗi | Mô tả lỗi, app đang gõ, vài chục dòng nhật ký gõ cuối (sửa được, xoá dòng riêng tư) và thông tin máy, rồi mở sẵn một issue trên GitHub. Chỉ mở trình duyệt khi bấm; issue là công khai |

Sửa trong cửa sổ có hiệu lực ngay ở phím gõ tiếp theo.

File người dùng nằm trong `~/Library/Application Support/Songngu/`: `memory.tsv` (lựa chọn đã học), `settings.tsv` (cài đặt, app bỏ qua/chặn phím), `typing-log.tsv` (nhật ký, nếu bật; ghi cả bundle id app mỗi khi đổi app và lý do mỗi lần bộ gõ tự chuyển một app sang chặn phím, để báo lỗi).

## Khắc phục sự cố

| Hiện tượng | Cách xử lý |
|---|---|
| Không thấy biểu tượng SN | System Settings > Menu Bar: bật mục Text Input/bộ gõ; thanh menu nhiều icon thì có thể bị tai thỏ che |
| Ctrl+Space không đổi bộ gõ, app không nhận bộ gõ, dấu đôi, nhảy chữ | Đang chạy bộ gõ chặn phím khác (xem **Bộ gõ khác** ở phần cài đặt; Cài đặt > Trạng thái báo tên app) |
| Terminal không ra dấu | Chưa có quyền Trợ năng (bước 3 phần cài đặt) |
| Một app (terminal lạ) hoàn toàn không ra tiếng Việt, thanh menu hiện A | App không đưa phím qua bộ gõ; Songngu tự chặn phím ở đó khi có quyền Trợ năng. Đã bật "Chỉ chặn phím ở app trong danh sách" thì thêm app ở Cài đặt > Ứng dụng > Chặn phím |
| Một app gõ ra chữ lặp hoặc sai | Menu SN > "Chặn phím ở app này" khi đang ở app đó |
| Biểu tượng hiện "A" ở một app | App đó không mở phiên bộ gõ; Songngu vẫn là bộ gõ đang chọn |

Tài liệu kỹ thuật: [kiến trúc](docs/architecture.md), [phát triển](docs/development.md), [đo đạc](docs/evaluation.md), [dữ liệu](docs/data.md).

License: MIT cho mã nguồn; từ điển và mô hình ngữ cảnh CC BY-SA 4.0, nguồn và ghi công xem [NOTICE](NOTICE).
