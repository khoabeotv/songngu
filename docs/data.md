# Dữ liệu

Từ điển đi kèm (`data/`):

| File | Nguồn | License |
|---|---|---|
| `vi_50k.txt` | [FrequencyWords](https://github.com/hermitdave/FrequencyWords) (Hermit Dave), OpenSubtitles 2018 | CC BY-SA 4.0 |
| `en_words.txt` | `en_50k.txt` của FrequencyWords, giữ từ chữ thường ≥2 ký tự mà bộ kiểm chính tả macOS (`NSSpellChecker`, en_US) chấp nhận ở dạng thường hoặc viết hoa đầu | CC BY-SA 4.0 (dẫn xuất) |
| `en_dev.txt` | Tự soạn: thuật ngữ công việc/công nghệ thiếu trong phụ đề | của dự án |

Bộ đo tiếng Anh/SQL (`tools/eval/data/en_sql_{dev,test}.txt`, 0,9k từ mỗi bộ): tự soạn, câu SQL và tin nhắn/commit/review tiếng Anh kiểu dân IT, chia xen kẽ dòng thành dev/test. Không dùng để học.
| `vi_extra.txt` | Tự ước lượng | của dự án |

- `vi_50k.txt` mất hầu hết âm tiết một chữ thuộc khối U+1Exx (ở, ạ, ừ, ồ…): bộ tách từ của FrequencyWords bỏ token một ký tự dạng NFC. `vi_extra.txt` bù số đếm ước lượng, cộng vào số đếm gốc.
- `vi_50k.txt` lẫn dạng NFD với số đếm riêng; khi nạp đã chuẩn hoá NFC rồi cộng dồn.
- Tần suất lấy từ phụ đề phim: thiếu từ công việc/IT và không phản ánh tần suất từ tiếng Anh chen trong câu tiếng Việt; phần đó học từ corpus ngữ cảnh.
- Viết tắt toàn phụ âm có đ (đc, sđt, bđs) lấy từ chính các danh sách tần suất tiếng Việt.

Corpus ngữ cảnh (`corpus/`, không commit). Chỉ dùng nguồn có license cho phép phân phối dữ liệu dẫn xuất, vì mô hình (bảng đếm) đóng vào app:

| Nguồn | License |
|---|---|
| [FineWeb-2](https://huggingface.co/datasets/HuggingFaceFW/fineweb-2) `vie_Latn`: phần test (học 6M từ + dev/test) và shard train `000_00000` (học thêm 21M từ: mọi câu chen tiếng Anh tới 10M từ, 0,6% câu thuần Việt) | ODC-By 1.0 (+ điều khoản Common Crawl) |
| Kỹ thuật (`corpus/tech/`, học 9,3M từ, dev 59k, test 60k): trang kỹ thuật/công việc của shard FineWeb-2 `000_00001` (ít nhất 10 thuật ngữ trong `en_dev.txt`, chiếm ≥1,5% số từ; bỏ trang viblo.asia) | ODC-By 1.0 (+ điều khoản Common Crawl) |
| Tài liệu dịch tiếng Việt trong `corpus/tech/`: [React](https://github.com/reactjs/vi.react.dev), [Vue](https://github.com/vuejs-translations/docs-vi), [Kubernetes](https://github.com/kubernetes/website) (`content/vi`) | CC BY 4.0 |
| Chat song ngữ do Claude Haiku 4.5 sinh (`corpus/chatgen/`: học 1,16M từ, dev 45k, test 51k) | output do dự án tạo (điều khoản Anthropic: output thuộc người dùng) |

Tạo lại `en_words.txt`: tải `https://raw.githubusercontent.com/hermitdave/FrequencyWords/master/content/2018/en/en_50k.txt` rồi lọc bằng `tools/data/en_words.m` (cách build ở đầu file).

Chat: Claude Haiku 4.5 sinh bằng prompt riêng của dự án (`PROMPT` trong `tools/data/gen_chat.workflow.js`), lọc bằng `tools/data/clean_chat.py` (bỏ emoji, tên người gửi đầu dòng, lời dẫn của model, tin trùng bộ khác). Phần học (`own.txt`) và phần đo dùng hai danh sách chủ đề khác nhau (124 và 40); phần đo chia theo chủ đề thành dev A (24k từ), dev B (21k) và test (51k). Không quá 0,3% số dòng (0,1% cụm 6 từ) của dev/test có cụm 6 từ nằm trong `own.txt`.

Kỹ thuật: tài liệu chia theo file 80/10/10, trang web theo md5(url) 90/5/5; dev/test là toàn bộ câu tài liệu của bộ đó cộng câu web ngẫu nhiên cho đủ 3000 câu, bỏ câu trùng phần học nào đó (`tools/data/build_tech.py`).

`test` của kỹ thuật, FineWeb và chat là bộ khoá: chỉ chạy để báo số. `corpus/chatgen/own_raw/` và `eval_raw/` giữ nguyên để dựng lại đúng mô hình.
