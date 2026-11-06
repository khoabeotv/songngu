# Đo đạc

## Người gõ giả lập (`src/typing.rs`)

Văn bản đo: mỗi dòng một tin nhắn, đáp án là chính chữ đó (so bỏ qua vị trí dấu thanh). Người gõ giả lập gõ từ tiếng Anh đúng từng chữ, từ tiếng Việt bằng Telex chuẩn (mũ/móc liền sau chữ, dấu thanh cuối từ) và không bao giờ tự chuyển ngôn ngữ.

- `--realistic`: 30% từ có thanh gõ dấu ngay sau nguyên âm (tieesng), 20% từ có ư/ơ/ă gõ w cuối (truongwf), 20% từ có â/ê/ô gõ phím mũ muộn hơn (thayas, canaf), 20% từ có đ gõ d thứ hai muộn hơn (duowdcj), 3% từ gõ nhầm một phím rồi Backspace ngay.
- `--typos R`: tỉ lệ R từ gõ nhầm không sửa (phím bên cạnh, sót, lặp, đảo phím). Từ đó không chấm vì sai đằng nào người gõ cũng phải sửa; chỉ đo ảnh hưởng lên các từ còn lại.
- `--seed N`: đổi ngẫu nhiên.

Từ sai đo qua lớp bộ gõ (`eval-session`; đường thay chữ và xoá lùi cho cùng số từ sai). Sửa-khi-Space và đổi giữa từ đo ở mức engine (`eval`).

## Kết quả (bộ test)

| Bộ test | Từ | Từ sai | Gõ như người thật | Sửa-khi-Space | Đổi giữa từ |
|---|---|---|---|---|---|
| Kỹ thuật (tài liệu dịch + trang kỹ thuật) | 60k | 0,02% | 0,03% | 0,07% | 0,41% |
| FineWeb (web đời thường) | 73k | 0,06% | 0,07% | 0,11% | 0,44% |
| Chat (Haiku sinh, chủ đề không có trong phần học) | 51k | 0,11% | 0,14% | 0,27% | 2,25% |

- Bỏ chat khỏi phần học: chat test sai 93 từ thay vì 54 (0,18%), kỹ thuật/FineWeb gần như không đổi (12/44 thay vì 11/42).
- Gõ nhầm 5% không sửa: các từ còn lại sai 55/14/40 (chat/kỹ thuật/FineWeb), như khi gõ sạch.
- Gõ dấu muộn đôi khi ra đúng một từ Anh (đi gõ `did`, thêm gõ `theme`, mây gõ `maya`): bộ gõ nghiêng tiếng Anh, Esc để lật. Phần lớn chênh lệch giữa cột gõ chuẩn và gõ như người thật ở chat là do những từ này.
- Ngữ cảnh học từ cùng nguồn với hai bộ test đầu (khác tài liệu); chat test cùng model sinh và prompt với phần học (khác chủ đề). Các số này lạc quan hơn văn bản thật của người dùng.
- Lỗi còn lại chủ yếu là từ có nghĩa ở cả hai thứ tiếng (been/bên, its/ít, car/cả), viết tắt trùng âm tiết Việt (ASO/ÁO, HW/HƯ) và lỗi chính tả có sẵn trong văn bản gốc (đuợc, nguời) mà Telex không gõ ra.
- Người dùng lật ngay từ sai: bộ nhớ riêng giảm số lần phải lật 12% (kỹ thuật dev, 17 -> 15) đến 21% (FineWeb dev, 43 -> 34); dev chat gần như không đổi (21 -> 19, 35 -> 35) (`eval --fix` so với `--no-learn`).
- Từ còn phân vân (Esc lật được ở từ trước con trỏ): 0,6-1,4% số từ sau khi gõ xong, 1,3-3,3% số phím lúc đang gõ dở (`eval-session`: unsure words, unsure keys).
- Tốc độ engine ~2 µs/phím, p99 ~5 µs; gõ đều 10 phím/giây ~100 µs/phím (bộ nhớ đệm nguội giữa hai phím; `target/bench` qua C ABI). Mô hình 22 MB, nạp ~1 ms; tiến trình bộ gõ ~9 MB footprint (gồm AppKit).

## Đã thử, không dùng

- Bộ xếp hạng lại bằng mạng nhỏ (`tools/eval/rerank_train.py`: embedding băm của 4 từ trước + 3 từ sau, học trên ví dụ `ambig` chấm chéo hai nửa corpus) cho từ phân vân: không hơn bước nhìn từ sau của bộ gõ trên dev. Lỗi còn lại cần hiểu nghĩa câu, tức một mô hình ngôn ngữ cỡ vài chục MB.
- Trigram: xem `architecture.md`.
