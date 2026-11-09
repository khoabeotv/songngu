# Kiến trúc

Không có chế độ Anh/Việt: mỗi phím chạy song song nhánh Telex và nhánh phím gốc, hiện chữ của nhánh có xác suất cao hơn. Mục tiêu: chữ đúng ngay lúc gõ, không phải đợi Space mới sửa, và sửa được bằng một phím khi đoán sai.

## Engine (`src/`)

- `syllable.rs`: tập âm tiết sinh từ luật ghép vần, đặt dấu thanh kiểu mới/cũ.
- `telex.rs`: ghép phím (Telex, VNI, `[ ]`), dấu gõ tự do thứ tự, gõ lặp để huỷ. "wu" là gõ đảo của "uw" khi còn nguyên âm theo sau (chwua -> chưa; vần "ưu" không có nguyên âm nào sau nó, nên hưu, cứu vẫn giữ). Đếm thao tác người gõ Telex hiếm làm (đổi dấu thanh giữa chừng, thêm mũ/móc sau dấu thanh) và dấu gõ muộn (mũ, đ không liền sau chữ của nó) để phạt nhánh Việt: "these" không thành "thế", "theme" vẫn là tiếng Anh (`odd_penalty`, `late_penalty`). Đ gõ muộn phạt nhẹ hơn (`late_d_penalty`): ít từ Anh trùng dạng, còn gõ "did" cho "đi" là thói quen phổ biến; ngữ cảnh quyết "toi did" -> "tôi đi", "I did it" giữ nguyên.
- `lexicon.rs`: mảng đã sắp xếp + tần suất cộng dồn, tiền tố = một đoạn liên tiếp, tra không cấp phát; viết tắt có đ (đc, sđt); bigram chữ cái cho từ Anh chưa gặp.
- `context.rs`: học từ văn bản song ngữ; bảng cặp từ CSR (số hiệu từ xếp theo dạng không dấu, 8 byte mỗi cặp, cờ lớp từ để cộng tiền tố không đọc chuỗi). Trigram đã thử (cắt ≥3/5/10): không đổi từ sai nào trên dev, file nặng thêm 10-21 MB, nên không dùng.
- `store.rs`: định dạng nhị phân của mô hình, mmap, kiểm biên và UTF-8 một lần lúc nạp.
- `engine.rs`: `P(w | trước) = λ·bigram + (1-λ)·P(tiếng | lịch sử)·P(w | tiếng)`, nhân hệ số chữ hoa và kiểu nối; giữa từ dùng cùng công thức cho tiền tố. Dấu ngăn cách chờ tới phím đầu của từ sau mới áp vào ngữ cảnh: dính liền hai từ thì `.` không phải hết câu. `P(tiếng | lịch sử)` trộn xác suất học từ corpus với tỉ lệ Anh/Việt 20 từ vừa gõ.
- Gõ đảo hai phím (`Engine::swap_repair`): phím không thành âm tiết Việt thì lúc chốt từ thử đảo từng cặp phím liền nhau, nhận từ có trong từ điển gõ đúng thứ tự (không đổi dấu thanh giữa chừng), nhân phạt 1e-5 rồi so với nhánh Anh như thường (nyaf -> này). Viết tắt toàn hoa không sửa. Chỉ sửa lúc chốt nên khi đang gõ vẫn thấy phím gốc. Lật bỏ từ đã sửa (`cso` là viết tắt chứ không phải "có") chỉ nhớ cặp từ ở ngữ cảnh đó, không trừ điểm từ Việt ở chỗ khác (`Memory::learn_pair`): từ đó do bộ gõ tự đoán, không phải người dùng gõ ra rồi chê.
- `memory.rs`: mỗi lần lật cộng cặp (từ trước, từ được chọn) với trọng số 20 lần gặp, và hệ số ưu tiên theo từ. Thói quen gõ: đếm từ Việt đã chốt có â/ê/ô (có đ) và bao nhiêu từ gõ mũ (đ) muộn; mức phạt mũ/đ muộn của người này là tỉ lệ đó, làm mượt về mức chung như đã thấy 50 từ, không thấp hơn mức chung. Chỉ lưu hai cặp số; từ chỉ được tính khi đã thôi lật được.
- `ime.rs`: logic phím độc lập hệ điều hành (nhìn từ sau, thay chữ trước con trỏ, mở lại từ, lật, phát hiện app không thay được chữ). `ffi.rs`: C ABI cho lớp OS. `settings.rs`: app bỏ qua/kiểu code/chặn phím (đánh dấu app bộ gõ tự thêm), tuỳ chọn.
- `typing.rs`: người gõ giả lập cho bộ đo (xem `evaluation.md`).

"Học" là đếm (không mạng neural):

- Tần suất từ trong miền, xác suất đổi thứ tiếng theo 2 từ trước, chữ hoa (viết tắt giữa câu tách khỏi tiêu đề viết hoa), cặp từ liền nhau.
- Kiểu nối với từ trước: cách bằng khoảng trắng hay dính liền bằng `.` `-` `_` (Nest.js, low-fidelity); theo tiếng từ trước vì phiên âm Việt cũng nối bằng `-`.
- Thứ tiếng của từ sau còn đếm theo từng từ trước, làm mượt về xác suất theo tiếng: sau "deal", "company" người Việt hay viết tiếp tiếng Việt ("deal sẽ close").
- Âm tiết Việt đúng luật mà danh sách tần suất thiếu được thêm nếu corpus gặp từ 2 lần (xịn xò); chưa gặp bao giờ (Bình Thạnh, bún riêu, giúm) thì vẫn là phương án với xác suất rất nhỏ (`unseen_vn`), để ngữ cảnh quyết.

`model.bin` gồm từ điển chung + ngữ cảnh, mọi bảng là mảng số căn 8 byte (`store.rs`). Bộ gõ mmap file: nạp ~1 ms, không parse, trang file chỉ đọc không tính vào bộ nhớ riêng của tiến trình. Thay file bằng đổi tên (`save-model` tự làm), không ghi đè tại chỗ khi bộ gõ đang chạy. Không có `model.bin` thì đọc `data/`, gõ không ngữ cảnh.

## Lớp bộ gõ (`src/ime.rs`, `platform/macos/`)

Không có vùng soạn (gạch chân): mỗi phím vào app ngay; engine đổi chữ ("tá" -> "task") thì xoá đúng phần khác trước con trỏ rồi chèn phần mới, bằng một trong hai đường:

- **Thay chữ** (mặc định, `SongnguController.m`): `insertText:replacementRange:` của app, không bắn Backspace giả nên không tranh với phím thật.
  - Đầu mỗi phím, bộ gõ so chữ trước con trỏ với chữ nó đã để lại. Khác sau phím thường = con trỏ đã bị click đi, bắt đầu từ mới. So ở phím sau chứ không ngay sau khi thay vì Chrome/Electron cập nhật chữ đọc được hơi trễ.
  - Ngay sau một lần thay mà chữ cũ còn nguyên và chữ mới bị chèn thêm ("taá"), 3 lần thay liền chữ trước con trỏ khác chữ bộ gõ để lại, hoặc 3 lần thay liền app không báo vị trí con trỏ = app không thay được: app đó chuyển hẳn sang chặn phím (ghi `tap … auto` trong `settings.tsv`, lý do vào nhật ký gõ). Khác đơn lẻ thường do macOS tự sửa chính tả hoặc tự viết hoa đúng lúc bộ gõ vừa thay (app gốc như Telegram), nên chỉ coi như con trỏ đã đi; chỉ khác chữ hoa thì không tính. Một lần lỡ nhịp (Flutter, Electron báo vị trí trễ ở đầu ô nhập) thì riêng lần đó sửa bằng phím giả.
  - App không cho đọc chữ (Zalo, vài app Flutter) thì thay theo vị trí con trỏ, không kiểm.
- **Chặn phím** (`KeyTap.m`; terminal, app tự phát hiện ở trên, menu "Chặn phím ở app này", và mọi phím rơi vào app đang không mở phiên nhập liệu với bộ gõ): bắt phím ở cấp phiên (CGEventTap, cần quyền Trợ năng), xoá bằng Backspace giả rồi gõ chữ mới bằng phím mang chuỗi Unicode; phím không đổi gì thì để phím thật đi qua.
  - Không đọc được chữ trong app nên chỉ mở lại được từ bộ gõ vừa đưa vào; click chuột, Enter, mũi tên kết thúc từ.
  - App không mở phiên nhập liệu (`activateServer`/`deactivateServer`; terminal của Herdr, vài app Flutter trên một số máy) thì phím không bao giờ tới lớp IMK: đường này tự lo, bỏ qua phím giữ lặp, và đánh dấu phím cho đi qua để IMK không xử lý lại nếu phiên mở muộn hơn phím. Chỗ không có ô nhập (danh sách thư, Finder) cũng rơi vào đây: gõ kiểu `dd`, `as` vẫn có thể sinh Backspace giả, như bộ gõ chặn phím. Tắt bằng tuỳ chọn `manual_tap`.
  - Chỉ chạy khi Songngu là nguồn nhập đang chọn. Chưa có quyền thì lớp IMK thử ký tự DEL qua `insertText` (Ghostty bỏ qua).
  - Bộ gõ chặn phím khác (XKey, EVKey...) chạy cùng thì hai bên giành phím: Ctrl+Space không đổi nguồn nhập, app không gửi phím qua bộ gõ.

Hành vi chung:

- **Nhìn từ sau**: từ còn phân vân (hai phương án chênh dưới 10⁴ lần) để từ sau đổi lại ("xin visa may mắn" -> "xin vía may mắn"). Từ sau phải làm phương án kia hơn 3 lần, tính cả xác suất đổi tiếng ("Tô subtle" -> "Too subtle") và kiểu nối ("lơ-fidelity" -> "low-fidelity"). Phương án kia là từ Anh hiếm trong corpus (dưới 100 lần có từ theo sau) mà chưa gặp từ sau sau phương án nào thì chỉ so xác suất theo tiếng: danh sách từ Anh lấy từ phụ đề lẫn từ ngoại/hiếm, ước lượng từ vài mẫu thổi phồng khả năng gặp từ mới ("nên model" không thành "nene model").
- **Lật**, theo thứ tự: từ đang gõ; lần từ sau vừa tự đổi từ trước (hoàn tác); từ vừa gõ xong (còn trong 4 dấu ngăn cách); từ ngay trước con trỏ (chỉ đường thay chữ). Bộ gõ nhớ 64 từ gần nhất cùng phím đã gõ; chữ khác (dán vào, gõ từ trước) thì chữ Latin thành Telex của nó, chữ Việt thành phím Telex với phím dấu thanh đặt chỗ cho ra từ Anh có thật ("tét" -> "test").
  - Backspace (mặc định): Backspace đầu tiên bấm ngay sau dấu ngăn cách vừa gõ thì lật lần vừa tự đổi, hoặc từ đó nếu hai phương án chênh dưới 1000 lần, hoặc đều là từ có trong từ điển và chênh dưới 10⁴ lần (`backspace_ratio`; "làm visa" ~10^3,7, còn "of" trong "the cost of" ~10^5,5 thì xoá). Khoảng 1,3-3% số từ; nới lên 10⁵ chỉ thêm 4/615 từ sai lúc chốt sửa được mà gấp đôi số lần lật thay vì xoá. Còn lại xoá như thường, nên "tiêng " + Backspace + s vẫn thành "tiếng". Esc trơn là của app.
  - Esc (tuỳ chọn `esc_flip`, app thường): lật chữ bộ gõ vừa đưa vào; từ trước con trỏ chỉ khi lúc gõ bộ gõ còn phân vân hoặc người dùng đã lật nó. Không có gì để lật thì Esc là của app.
  - Bấm lại chính phím vừa lật trong 1 giây (`esc_undo_window`): trả lại chữ, bỏ điều vừa học, đưa phím cho app (Backspace xoá, Esc đóng hộp thoại). Giữ Backspace để xoá cả đoạn vẫn xoá đúng.
  - Ctrl+Esc / Shift+Esc (`Flip`): mọi app, lật cả chữ bộ gõ không gõ ra. Terminal/IDE chỉ có cách này.
- **Gõ lại là lật**: xoá hết một từ mà hai phương án đều đáng kể (đang gõ dở, hoặc từ vừa chốt sau khi đã xoá dấu ngăn cách) rồi gõ ngay ra đúng cặp phương án đó thì ép phương án còn lại, ghi nhớ như một lần lật. Gõ khác đi hoặc sang từ khác thì thôi.
- Mỗi lần lật được nhớ vào `memory.tsv` (`memory.rs`).
- Backspace trong từ xoá chữ đang hiện, giữ dấu ("tiếng" -> "tiến"); chỉ mất chữ cuối thì để chính phím Backspace đi vào app. Gõ ngay sau một từ đã có thì mở lại từ đó ("tiêng" + s -> "tiếng"). Không đọc được chữ (chặn phím, ô không cho đọc) thì chỉ mở lại từ vừa gõ khi đã xoá hết dấu ngăn cách sau nó (gõ "tieeng ", Backspace, s -> "tiếng").
- Enter, Tab, mũi tên, phím tắt, click chuột: kết thúc từ (sửa nốt chữ nếu cần) rồi để app xử lý. Ngữ cảnh mất theo: phím đầu của từ sau đọc chữ trước con trỏ (app cho đọc), lấy từ cuối làm từ trước và tính tiếng của tối đa 8 từ cuối vào tỉ lệ Anh/Việt gần đây (`Engine::resume`). Hết câu trước con trỏ thì vẫn là đầu câu; từ dính sát con trỏ thì mở lại từ đó như trên.
- Terminal/IDE (danh sách trong `src/settings.rs`) nghiêng tiếng Anh ×5, Esc là của app.
- Engine lỗi (panic) thì bộ gõ tự tắt, phím đi thẳng vào app.

## macOS

- Cửa sổ cài đặt (SwiftUI, `platform/macos/settings/`) là app riêng ở `Contents/Helpers/SongnguSettings.app`, menu SN mở nó. Nó đọc/sửa file người dùng qua C ABI của thư viện Rust (`ffi_settings.rs`), để cách đọc file chỉ nằm một chỗ. Hai tiến trình chỉ nói chuyện qua file: cửa sổ ghi trọn `settings.tsv`/`memory.tsv` (file tạm rồi đổi tên), bộ gõ thấy thời điểm sửa file đổi thì nạp lại (khi đổi app, mở menu, trước khi tự ghi, và tối đa mỗi 500 ms khi gõ, `ffi.rs`). Bộ gõ ghi `status.tsv` (pid, quyền Trợ năng, chặn phím) khi trạng thái đổi, vì cửa sổ là tiến trình khác, không tự hỏi được quyền của bộ gõ. Cửa sổ không có ô nhập chữ nên đường chặn phím bỏ qua nó.
- Bộ gõ chạy nền khác bắt phím bằng event tap: hai bên cùng sửa một phím. `platform/macos/conflicts.tsv` (bundle id, tên, tên tiến trình; đóng vào Resources) là danh sách duy nhất mà `install.sh` và trang Trạng thái dùng để cảnh báo khi thấy chúng đang chạy; chỉ cảnh báo, không tự tắt. Bộ gõ InputMethodKit không cần vì mỗi lúc chỉ một nguồn nhập được chọn.
- Tự cập nhật (bản cài bằng `install.sh`, tuỳ chọn `auto_update`): bộ gõ không truy cập mạng, chỉ mỗi giờ gọi `SongnguSettings --update` chạy ngầm; cửa sổ cài đặt tự giới hạn mỗi ngày một lần (`update.tsv`), đọc phiên bản và checksum trong `install.sh` của release Latest, tải zip, kiểm checksum và kiểm bản mới thoả designated requirement của `Songngu.app` đang cài (cùng chứng chỉ "Songngu Dev"), rồi đổi tên thay bundle và tắt tiến trình bộ gõ cũ. Chiếm được GitHub cũng không đẩy được bản giả vì khoá ký chỉ nằm trên máy phát hành; bản đang cài ký ad-hoc (build từ mã nguồn không có chứng chỉ) thì không tự cập nhật.
- `Info.plist` khai báo nguồn nhập; biểu tượng `icon.pdf` (vẽ bằng `mkicon.m`) là ảnh template 20×16 pt, hệ thống tô theo giao diện sáng/tối.
- `build.sh` ký bằng chứng chỉ "Songngu Dev" nếu có trong keychain: quyền Trợ năng gắn với chứng chỉ nên còn nguyên qua các lần build. Không có thì ký ad-hoc, mỗi lần build phải cấp lại quyền.
- Phát hành cho người khác: cần ký Developer ID (`codesign --options runtime --sign "Developer ID Application: …"`), notarize (`xcrun notarytool submit … --wait`) và `xcrun stapler staple`; xem lại điều kiện phân phối mô hình trong `NOTICE`.
- Ghi vết gỡ lỗi: tạo file `~/Library/Application Support/Songngu/trace` rồi khởi động lại bộ gõ; `trace.log` cạnh đó ghi mọi sự kiện kèm chữ gõ. Xoá cả hai khi xong.
