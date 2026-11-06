# Songngu

Bộ gõ song ngữ Việt/Anh cho macOS, không có chế độ Anh/Việt: cứ gõ Telex, bộ gõ tự hiểu từ nào là tiếng Việt, từ nào là tiếng Anh ngay lúc gõ ("truowfng merge test cos" -> "trường merge test có"). Đoán sai thì bấm Backspace ngay sau từ đó để lật.

## Cài đặt

Cần macOS 12 trở lên, Xcode Command Line Tools (`xcode-select --install`) và Rust (`curl https://sh.rustup.rs -sSf | sh`).

1. Build và cài:

   ```sh
   platform/macos/build.sh --install
   ```

   Lần đầu build tự tải mô hình ngữ cảnh (22 MB) từ mục Releases của repo về `models/model.bin` rồi đóng vào app. Repo private thì cần đăng nhập GitHub CLI trước (`brew install gh && gh auth login`). Không tải được thì bộ gõ vẫn chạy nhưng đoán kém hơn.

2. Bật: System Settings > Keyboard > Text Input > Edit > **+** > Vietnamese > **Songngu**. Biểu tượng **SN** hiện trên thanh menu; Ctrl+Space đổi qua lại với ABC.

3. Gõ trong terminal (Ghostty, Terminal, iTerm2...) cần quyền Trợ năng: System Settings > Privacy & Security > Accessibility, thêm `~/Library/Input Methods/Songngu.app` (kéo thả từ Finder, hoặc menu SN > "Cấp quyền Trợ năng…") rồi bật.

   Mỗi lần build lại, quyền này mất hiệu lực, trừ khi có chứng chỉ ký mã tên **Songngu Dev**: Keychain Access > Certificate Assistant > Create a Certificate, Identity Type "Self Signed Root", Certificate Type "Code Signing". Có chứng chỉ thì `build.sh` tự ký bằng nó và quyền giữ nguyên qua các lần cập nhật.

**Cập nhật**: `git pull` rồi chạy lại `platform/macos/build.sh --install`, chuyển sang ABC rồi quay lại Songngu.

**Gỡ**: bỏ Songngu khỏi danh sách Text Input, xoá `~/Library/Input Methods/Songngu.app` và `~/Library/Application Support/Songngu`.

## Sử dụng

**Gõ**: Telex, dấu đặt ở đâu trong âm tiết cũng được: `tieesng`, `tieengs`, `tienges` đều ra "tiếng"; `truowngf`, `truongwf`; `thaays`, `thayas`; `dduwowcj`, `duodwcj`. Từ tiếng Anh gõ như bình thường. Gõ lặp phím dấu để huỷ dấu đó và giữ chữ cái.

**Lật Anh/Việt** khi bộ gõ đoán sai:

| Phím | Lật gì |
|---|---|
| **Backspace** ngay sau từ (sau dấu cách hoặc dấu câu vừa gõ) | Từ vừa gõ, nếu cả hai phương án đều là từ có thật (`did`/đi, `theme`/thêm, `visa`/vía); hoặc lần bộ gõ vừa tự sửa từ trước. Từ khác thì Backspace xoá như thường |
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
| Bỏ qua app này | Songngu đứng ngoài app đang mở, phím đi thẳng vào app |
| Chặn phím ở app này | Sửa chữ bằng Backspace giả thay vì thay chữ trực tiếp. Tự bật cho terminal và app không cho thay chữ; tự bật tay khi một app gõ ra chữ lặp ("taá") |
| Đặt dấu kiểu cũ | hòa, thủy thay vì hoà, thuỷ |
| Gõ VNI | Phím số làm dấu |
| `[ ]` gõ ơ ư | `[` = ơ, `]` = ư |
| Lật Anh/Việt bằng Esc thay cho Backspace | Backspace chỉ còn là xoá; Esc lật ở app thường |
| Chỉ chặn phím ở app trong danh sách | Tắt việc tự chặn phím ở app không nhận bộ gõ. Mặc định Songngu tự lo các app này (như XKey), đổi lại gõ chữ ở chỗ không có ô nhập (danh sách thư, Finder) có thể sinh Backspace giả |
| Ghi nhật ký gõ | Ghi phím và chữ ra `typing-log.tsv` trên máy này (để báo lỗi), không ghi trong ô mật khẩu |
| Quên những gì đã học | Xoá bộ nhớ lật; file cũ giữ ở `memory-forgotten.tsv` |

File người dùng nằm trong `~/Library/Application Support/Songngu/`: `memory.tsv` (lựa chọn đã học), `settings.tsv` (cài đặt, app bỏ qua/chặn phím), `typing-log.tsv` (nhật ký, nếu bật).

## Khắc phục sự cố

| Hiện tượng | Cách xử lý |
|---|---|
| Không thấy biểu tượng SN | System Settings > Menu Bar: bật mục Text Input/bộ gõ; thanh menu nhiều icon thì có thể bị tai thỏ che |
| Ctrl+Space không đổi bộ gõ, app không nhận bộ gõ | Đang chạy bộ gõ chặn phím khác (XKey, EVKey...): thoát nó và gỡ khỏi Login Items |
| Terminal không ra dấu | Chưa có quyền Trợ năng (bước 3 phần cài đặt) |
| Một app (terminal lạ) hoàn toàn không ra tiếng Việt, thanh menu hiện A | App không đưa phím qua bộ gõ; Songngu tự chặn phím ở đó khi có quyền Trợ năng. Đã bật "Chỉ chặn phím ở app trong danh sách" thì thêm tay vào danh sách chặn phím, `printf 'tap\t<bundle id>\n' >> ~/Library/Application\ Support/Songngu/settings.tsv; pkill -x Songngu` (bundle id: `osascript -e 'id of app "Tên App"'`) |
| Một app gõ ra chữ lặp hoặc sai | Menu SN > "Chặn phím ở app này" khi đang ở app đó |
| Biểu tượng hiện "A" ở một app | App đó không mở phiên bộ gõ; Songngu vẫn là bộ gõ đang chọn |

Tài liệu kỹ thuật: [kiến trúc](docs/architecture.md), [phát triển](docs/development.md), [đo đạc](docs/evaluation.md), [dữ liệu](docs/data.md).

License: MIT cho mã nguồn; từ điển và mô hình ngữ cảnh CC BY-SA 4.0, nguồn và ghi công xem [NOTICE](NOTICE).
