# viet-core

Engine gõ tiếng Việt Rust thuần, xuất `rlib` và `cdylib`, chuyển thể từ
[BambooEngine/bamboo-core](https://github.com/BambooEngine/bamboo-core)
commit `b2e49a2b48c7d3772a3673142a7747eccd9d5f79` (MIT).
Giữ thông báo bản quyền nguồn trong `LICENSE`. Không dùng Go runtime/cgo.

## Build và sử dụng

```sh
cargo test --release
cargo build --release
```

Linux tạo `target/release/libviet_core.so`, Windows tạo `target/release/viet_core.dll`.
Header C/C++: `include/viet_core.h`. Mỗi engine dùng riêng cho một input context;
frontend phải tuần tự hóa lời gọi trên cùng handle. Chuỗi trả về thuộc caller,
giải phóng bằng `viet_string_free`; handle giải phóng bằng `viet_engine_free`.
`viet_engine_key` nhận Unicode scalar, không nhận X11/IBus keysym.
Frontend xử lý các phím điều khiển, commit/preedit, ranh giới từ và reset engine.
Kho chứa này chưa có adapter IBus/Fcitx5 hay kiểm thử tích hợp desktop Linux.

```rust
use viet_core::Engine;
let mut engine = Engine::default();
engine.process("tieengs Vieetj");
assert_eq!(engine.text(), "tiếng Việt");
```

`Engine::text()` và `raw()` trả về toàn composition. `reset()` sau commit để giới
hạn bộ nhớ. `backspace(true)` xóa ký tự hiển thị cùng các phép biến đổi liên quan,
không chỉ xóa phím vật lý cuối. `restore_last_word(false/true)` khôi phục phím gốc/
xử lý lại tiếng Việt. Arena giữ chỉ số mục tiêu ổn định trong mỗi thao tác;
các node thử nghiệm được loại sau mỗi phím.

`src/syllable.rs`: transformation arena, Unicode, tách CVC và ngoại lệ gi/qu.
`src/rules.rs`: luật DSL, các bảng chính tả và bitmask quan hệ âm đầu/nguyên âm/âm cuối.
`src/engine.rs`: tìm mục tiêu, undo, free-mark, shortcut uow, dịch dấu.
Giữ cách tách CVC của Bamboo: âm đệm nằm trong các chuỗi nguyên âm, không có cây
âm tiết/trie hoặc enum Labial riêng trong nguồn Go này.

Flags giống Go: bit 1 (`FREE_MARK`), bit 2 (`STANDARD_TONE`), bit 4 dự phòng theo
`EautoCorrectEnabled` nguồn Go (nguồn được pin không dùng bit này trong xử lý).
Mặc định 7 cho `hòa`, `ủy`; 5 cho `hoà`, `uỷ`. Tên “cũ/mới” trong yêu cầu không
được dùng làm tên API để tránh nhầm với tên cờ của Bamboo.
Toàn bộ 9 bảng gõ có sẵn trong `src/input_methods.json`, truy cập qua
`InputMethod::builtin(name)`. `InputMethod::from_definition` nhận bảng DSL tùy chỉnh.

## Dữ liệu Bamboo

`MacroTable::load(path, ignore_case)` đọc `key:value`, bỏ dòng comment `;`/`#`,
trim hai phía, mục sau ghi đè mục trước. Giống Bamboo, dòng có hơn một dấu `:`
bị bỏ qua. Đọc cả dòng cuối không có newline. Chỉ tra cứu dữ liệu; frontend quyết
định thời điểm mở rộng macro và chuyển hoa.

`BambooConfig::load(path)` đọc JSON cấu hình từ
`~/.config/ibus-bamboo/ibus-bamboo.config.json`; macro thường ở
`~/.config/ibus-bamboo/ibus-bamboo.macro.text`.
Danh sách loại trừ là `InputModeMapping` với mode **7** (`UsIM`), so khớp chính xác
WM_CLASS như Bamboo, không phải file blacklist riêng. `is_excluded` dùng
`DefaultInputMode` khi không có mapping hợp lệ. `engine()` sử dụng InputMethod,
Flags và InputMethodDefinitions trong cấu hình. Parser định dạng được viết riêng;
không sao chép mã frontend GPL của ibus-bamboo.

## Kiểm chứng và giới hạn

Tất cả tệp kiểm thử, fixture và công cụ hỗ trợ nằm trong `tests/`;
script/công cụ đặt trong `tests/tools/`. Không tạo tệp kiểm thử rải rác bên ngoài
thư mục này. Quy ước làm việc được ghi trong `AGENTS.md`.

`tests/go_reference.json`: 22.548 ca, 158.580 trạng thái sau phím, sinh từ Go đã pin,
so sánh text/raw/IsValid(false) trên Telex, Telex 2, VNI, VIQR và flags 7/5/0;
bao gồm chuỗi ngẫu nhiên xác định seed và Backspace. Chạy fixture không cần Go.
Đây là bằng chứng trên tập kiểm thử, không phải chứng minh tương đương cho mọi
chuỗi Unicode hay mọi bảng DSL tùy chỉnh. Chưa benchmark hoặc tối ưu để khẳng định
hiệu năng tối đa. Chưa port các output charset legacy, encoder hoặc toàn bộ Mode
bitmask của Go; đầu ra thư viện này là Unicode UTF-8.

Tái sinh fixture: clone nguồn Go vào `upstream/bamboo-core`, checkout commit trên,
Chạy `go build -o ../../tests/tools/oracle.exe ../../tests/tools/oracle.go` từ
clone rồi chạy `python tests/tools/make_fixtures.py` từ repo.
Go chỉ phục vụ tạo dữ liệu kiểm thử.
Nguồn cấu hình tham chiếu: ibus-bamboo commit
`49e03d7a36b8a8b796950c158b545af8cf9707d4`.
