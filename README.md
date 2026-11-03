# viet-ime

Bộ gõ tiếng Việt theo mô hình Rust core độc lập và hai adapter mỏng cho Linux.
Core sở hữu thuật toán, trạng thái, cấu hình, chính sách ứng dụng và IPC. Wrapper
không đọc cấu hình người dùng và không xử lý luật tiếng Việt.

```text
core-rust/
  src/composition.rs        thuật toán Transformation đã đối chiếu Go
  src/syllable.rs           Unicode, liên kết mục tiêu, tách CVC/gi/qu
  src/rules.rs              luật DSL, ma trận chính tả, 9 bảng gõ
  src/engine.rs             xử lý key, app profile, action
  src/app_profile.rs        chính sách output theo App ID/WM_CLASS
  src/settings.rs           cấu hình TOML do core đọc
  src/ipc.rs                Unix socket và cấu hình chia sẻ trong tiến trình
  src/ffi.rs                đúng 5 symbol C, kết quả chuỗi mượn
  include/viet_core.h        hợp đồng ABI cho cả hai framework
wrappers/ibus-c/engine.c     IBusEngine C
wrappers/fcitx5-cpp/engine.cpp  InputMethodEngineV2 C++
tests/                      toàn bộ test, fixture, công cụ và dữ liệu tham chiếu
```

Core không liên kết IBus, Fcitx5, X11 hoặc Wayland; không dùng Go runtime/cgo.
Thuật toán được chuyển thể từ [bamboo-core](https://github.com/BambooEngine/bamboo-core)
commit `b2e49a2b48c7d3772a3673142a7747eccd9d5f79` (MIT, thông báo trong LICENSE).
Giữ các phép biến đổi và mục tiêu trong arena; hỗ trợ free-mark, undo, dịch dấu,
Backspace và các ngoại lệ `gi`, `qu`, `ươi`, `uôi`, `uya`.

## Cài nhanh trên Linux

Từ thư mục mã nguồn đã tải, chạy bằng tài khoản thường:

```sh
sh install.sh
```

Bộ cài tự chọn framework từ môi trường input method, tiến trình đang chạy rồi
desktop: GNOME ưu tiên IBus; KDE/XFCE/Hyprland ưu tiên Fcitx5 khi chưa có tín hiệu
phiên làm việc khác. Tự cài dependency bằng apt, dnf, pacman, zypper hoặc apk; chỉ
build wrapper đã chọn. Nếu Rust distro quá cũ, dùng rustup chính thức, kiểm SHA-256,
không sửa shell profile. Cần mạng khi tải dependency; SDK IBus cần >= 1.5.27.

Quyền sudo/doas chỉ dùng để cài package và chép sản phẩm vào hệ thống; build chạy
với tài khoản gọi script. Sau khi cài, đăng xuất/đăng nhập và thêm **Viet IME** trong
phần Input Sources/Input Method của desktop. Script không ghi đè input sources,
environment hoặc tự restart phiên đang làm việc. Không thể chọn input method thay
người dùng một cách đồng nhất cho mọi GNOME/KDE/Wayland compositor.

```sh
sh install.sh --dry-run                   # xem kế hoạch, không ghi/tải/chạy sudo
sh install.sh --framework ibus            # chọn rõ framework
sh install.sh --framework fcitx5
sh install.sh --framework both            # chỉ dùng khi cần cả hai
sh install.sh --no-deps --build-only      # môi trường đã có toolchain/SDK
sh install.sh --jobs 2                    # giảm tải RAM khi build
sh install.sh --package deb               # tạo gói Debian/Ubuntu, chưa cài
sh install.sh --package rpm               # tạo gói Fedora/openSUSE, chưa cài
sh install.sh --package tar               # tạo tarball theo máy đang build
```

Build tại `target/linux-install/<framework>/`; gói nằm trong `packages/` dưới
thư mục build. CPack tự dò phụ thuộc ELF cho DEB/RPM; máy tạo gói cần
`dpkg-shlibdeps` hoặc `rpmbuild`. File `.deb`/`.rpm` có thể cài và gỡ bằng package
manager chuẩn. Tarball vẫn cần runtime framework, không phải AppImage độc lập.
Mặc định script cài thẳng bằng CMake; muốn quản lý/gỡ bằng package manager thì dùng
`--package deb/rpm`, rồi cài gói tương ứng. Chưa có sẵn binary release để tải.

Thư viện core cài riêng vào `<libdir>/viet-ime`, plugin dùng `$ORIGIN` RPATH nên
không cần sửa `LD_LIBRARY_PATH` hoặc chạy ldconfig. Fcitx5 tìm thư mục addon từ SDK
máy đích, hỗ trợ `lib`, `lib64`, multiarch. Có thể đặt `--prefix` riêng, nhưng khi
đó phải cấu hình framework tìm metadata/plugin trong prefix đó.

Mục tiêu là giảm thao tác trên các distro có package manager thông thường, không
khẳng định một binary chạy trên mọi Linux. NixOS/Guix, Fedora Atomic/immutable và
framework chạy trong Flatpak cần đóng gói theo hệ thống đó; script không tự ghi vào
`/usr` của chúng. Distro lạ có thể chuẩn bị dependency rồi dùng `--no-deps`.
Package names và kế hoạch đã kiểm thử tự động; cài thật trên từng distro, GUI GTK
và tạo DEB/RPM vẫn cần kiểm chứng trong môi trường Linux.

## Một bảng điều khiển chung

Mở **Viet IME Settings** từ menu ứng dụng hoặc chạy `viet-config --gui`.
Đây là hộp thoại GTK qua Zenity, dùng chung cho IBus/Fcitx5; chưa có system tray.
Đổi Telex/VNI, bật/tắt tiếng Việt và kiểu dấu rồi lưu. Phần đọc/ghi/kiểm tra cấu hình
nằm trong Rust core, không nằm trong wrapper. GUI lưu TOML bằng thay thế atomic,
giữ các rule ứng dụng và liên kết dữ liệu Bamboo, rồi gửi reload tới mọi socket core
trong phiên. Core chưa chạy thì cấu hình vẫn được lưu để dùng lúc khởi động sau.
Việc lưu chuẩn hóa lại TOML, giữ giá trị cấu hình nhưng không giữ comment/format cũ.

CLI dùng cùng module, không cần GUI:

```sh
viet-config --status                # xem cấu hình lưu trên đĩa
viet-config --set-method VNI
viet-config --enable
viet-config --disable
viet-config --reload
```

Không có một service gõ riêng bắt buộc phải bật: listener nằm trong tiến trình
IBus/Fcitx5 đang nhúng core. GUI chỉ là controller độc lập. C ABI vẫn đúng năm hàm.

## Build

Core trên Windows hoặc Linux:

```sh
cargo test --release --locked
cargo build --release --locked
python3 tests/ffi_smoke.py
```

Linux tạo `target/release/libviet_core.so`; Windows tạo `viet_core.dll`.
Windows chạy `python` thay `python3`. Build core cần Rust >= 1.85. Build hai wrapper trên Linux cần CMake >= 3.16,
pkg-config, C/C++ compiler, IBus >= 1.5.27 và Fcitx5 development packages:

```sh
# Ví dụ Debian/Ubuntu, cài dependencies qua package manager nếu chưa có:
# sudo apt install build-essential cmake pkg-config libibus-1.0-dev libfcitx5core-dev libfcitx5utils-dev
cmake -S . -B tests/artifacts/linux-build -DCMAKE_INSTALL_PREFIX=/usr
cmake --build tests/artifacts/linux-build --parallel
# Chỉ khi muốn cài vào hệ thống:
# sudo cmake --install tests/artifacts/linux-build
```

`-DVIET_BUILD_IBUS=OFF` hoặc `-DVIET_BUILD_FCITX5=OFF` để build một framework.
Metadata đăng ký IBus/Fcitx5 đi kèm. Sau khi cài, restart framework và thêm **Viet IME**.
Không chạy đồng thời hai framework trên cùng input context.
Hai file adapter đều dưới 250 dòng. Rust core đã build/test trên Windows và kiểm tra
biên dịch cả nhánh Unix bằng target `x86_64-unknown-linux-gnu`; việc build và smoke-test
hai adapter với framework thật trên Linux chưa được thực hiện trong môi trường này.
`sh tests/run-linux.sh` cung cấp các bước kiểm tra Linux, gồm test socket và build wrapper.

## C ABI: đúng 5 hàm

```c
VietEngine *viet_engine_new(void);
void viet_engine_free(VietEngine *engine);
void viet_engine_set_app_id(VietEngine *engine, const char *app_id);
CActionResult viet_engine_process_key(VietEngine *engine, uint32_t keyval,
                                     uint32_t keycode, uint32_t modifiers);
void viet_engine_reset(VietEngine *engine);
```

Đây là ABI mới, thay ABI thử nghiệm trước đó. `CActionResult` chứa `action_type`,
`backspace_count`, `preedit_str`, `commit_str`. Chuỗi UTF-8 do handle sở hữu, hiệu lực
đến lời gọi thay đổi tiếp theo trên cùng handle hoặc khi free. Caller không giải
phóng chuỗi; không cần hàm free-result thứ sáu. Một handle cho mỗi input context,
không gọi đồng thời/reentrant trên một handle. Handle khác có composition riêng.
`new()` trả NULL nếu khởi tạo cấu hình/socket lỗi; wrapper cho phím đi thẳng khi NULL.

| Action | Wrapper thực hiện |
|---|---|
| Ignore | Vẫn áp dụng chuỗi preedit/commit trả về, rồi cho phím gốc đi qua |
| Preedit | Thay toàn preedit; có thể kèm commit của composition trước khi đổi cấu hình |
| Commit | Xóa preedit và commit chuỗi mới |
| DeleteSurrounding | Xóa `backspace_count` ký tự trước cursor, rồi commit suffix |
| FakeBackspace | Forward `backspace_count` cặp Backspace press/release, rồi commit suffix |

`backspace_count` đếm Unicode scalar, không phải byte UTF-8. Core chỉ thay suffix sau
tiền tố chung để giảm số lần xóa. Các action khác Ignore tiêu thụ phím gốc, kể cả
Commit với chuỗi rỗng. Wrapper không tự tính dấu, diff hoặc quyết định mode.

Phím vào là keysym IBus/Fcitx/XKB: Latin-1 hoặc `0x01000000 | Unicode scalar`.
Các phím Backspace, Escape, Return, Tab, điều hướng do core phân loại. `keycode`
được dành cho metadata framework, hiện không dùng để quyết định thuật toán.
Modifier phải chuyển về các bit `VIET_*` trong header; không truyền bitmask framework
trực tiếp. Release/synthetic forwarded events không cập nhật output hoặc composition.

Khi focus-in, truyền App ID vào `set_app_id`; khi focus-out, gọi `process_key` với
`VIET_KEY_FLUSH`, áp dụng kết quả, rồi `reset`. Flush có thể trả Ignore cùng commit
để chốt preedit; sentinel không được chuyển tới ứng dụng. `reset` đơn thuần bỏ
composition. Gọi reset khi ứng dụng thay đổi cursor/selection/text bên ngoài core;
framework phải cung cấp tín hiệu đó. Phím điều hướng/phím tắt được core tự chốt từ
và trả Ignore để framework chuyển tiếp. Password/sensitive fields truyền `VIET_SENSITIVE`.

## Chính sách ứng dụng

Ba output mode: `Preedit`, `SurroundingText`, `BackspaceCommit`.
Mapping người dùng so khớp chính xác, có phân biệt hoa thường, được ưu tiên cao nhất.
Alias mặc định so khớp tên đầy đủ hoặc thành phần WM_CLASS/client-name, không dùng
substring hoặc wildcard. Không suy đoán mọi app Electron/Wine từ một tên chung.

| Nhóm mặc định | Mode yêu cầu |
|---|---|
| Firefox, Chrome, Chromium | SurroundingText |
| VS Code/Codium, Telegram | BackspaceCommit |
| Terminal và ứng dụng chưa biết | Preedit |

Wrapper báo `VIET_CAP_SURROUNDING` theo capabilities của client; nếu thiếu, core
chuyển SurroundingText sang BackspaceCommit. Core không tự truy cập framework để
thăm dò capabilities. Các tên trình duyệt tham khảo `DefaultBrowserList` của
[ibus-bamboo/utils.go](https://github.com/BambooEngine/ibus-bamboo/blob/49e03d7a36b8a8b796950c158b545af8cf9707d4/utils.go).
Các lựa chọn mode/alias IDE và terminal là chính sách bổ sung của viet-ime, không
phải bảng mặc định upstream được kiểm chứng cho mọi phiên bản ứng dụng.

IBus `focus-in-id` cung cấp client type/name, không bảo đảm WM_CLASS hay App ID thực.
`gtk3-im:firefox` có thể nhận diện; `Qt`, `xim`, `gnome-shell` chung dùng mặc định.
Adapter này không mở X11/compositor connection để đoán cửa sổ đang active.
Fcitx5 dùng `InputContext::program()` và trạng thái riêng theo InputContextProperty,
theo mô hình lifecycle của Lotus.

FakeBackspace ở đây dùng forwarding API của framework, không phải XTest/uinput.
Wrapper gửi các lệnh xóa trước commit; chưa có hàng đợi xác nhận hoàn tất async,
native uinput hay cơ chế delay/ack đầy đủ như Lotus. App ID cũng không phân biệt
thanh địa chỉ với ô soạn thảo trong cùng trình duyệt. Vì vậy đây là nền tảng action
và adapter mẫu, chưa phải cam kết hết lỗi nhảy chữ trên mọi ứng dụng.

## Cấu hình và IPC do core sở hữu

Copy nội dung `core-rust/config.example.toml` vào
`~/.config/viet-ime/config.toml` (hoặc `$XDG_CONFIG_HOME/viet-ime/config.toml`).
Không có file thì dùng mặc định; cấu hình sai bị từ chối. Core không ghi đè cấu hình
người dùng. Hỗ trợ `input_method = "Telex" / "Telex 2" / "VNI"`, `enabled`, `flags`,
`[apps]` và `[apps.rules]`; rule `"Disabled"` cho ứng dụng loại trừ.
Flags 7 cho `hòa`, `ủy`; 5 cho `hoà`, `uỷ`; bit 4 giữ chỗ theo nguồn Go.

`bamboo_config` tùy chọn trỏ tới JSON cũ: import `InputModeMapping` và
`DefaultInputMode`, không import tùy chọn framework. Mode 1 -> Preedit, 2 ->
SurroundingText, 3..6 -> BackspaceCommit, 7 -> Disabled; rule TOML trùng key thắng.
Mode 3..6 được hợp nhất theo kiến trúc ba mode mới, không mô phỏng Shift+Left hoặc
forward Unicode của ABI cũ. Parser macro `config::MacroTable` vẫn đọc dữ liệu
`key:value` của Bamboo; dispatcher hiện chưa tự mở rộng macro.

Mỗi tiến trình host có một socket:
`$XDG_RUNTIME_DIR/viet-ime/core-<pid>.sock`. Không có XDG_RUNTIME_DIR thì dùng
`<config-dir>/run/viet-ime/core-<pid>.sock`. Thư mục riêng mode 0700, socket 0600.
Nhiều handle chia sẻ cấu hình, không chia sẻ trạng thái từ. Có thể tồn tại một socket
cho IBus và một socket cho Fcitx5; controller gửi đến từng socket nếu muốn đồng bộ cả hai.

Protocol: một JSON UTF-8 kết thúc bằng newline trên mỗi connection, tối đa 4096 byte,
deadline đọc 100 ms; trả một JSON có `ok`, `revision`, `settings` hoặc `error`.

```json
{"command":"status"}
{"command":"set_input_method","value":"VNI"}
{"command":"set_enabled","value":false}
{"command":"toggle"}
{"command":"reload"}
```

Ví dụ: `printf '%s\n' '{"command":"set_input_method","value":"VNI"}' | socat - UNIX-CONNECT:"$XDG_RUNTIME_DIR/viet-ime/core-1234.sock"`.
Lệnh set/toggle áp dụng trong bộ nhớ; reload đọc lại TOML. Cấu hình được lấy ở sự kiện
phím tiếp theo; preedit cũ được commit trước khi bắt đầu composition mới, text đã
commit trong fallback được giữ nguyên. Thread IPC không gọi framework hoặc sửa
composition từ nền. Khi handle cuối cùng được free, listener dừng/join và socket
được dọn. Windows vẫn chạy core/dispatcher, không mở Unix listener.

## Kiểm chứng và nguồn tham chiếu

Tất cả tệp test, fixture, script, binary hỗ trợ và dữ liệu tham chiếu nằm dưới
`tests/`, theo `AGENTS.md`; sản phẩm Cargo nằm trong `target/`.
`tests/go_reference.json` có 22.548 ca, 158.580 trạng thái sau phím so với Go đã pin:
text/raw/IsValid(false), Telex/Telex 2/VNI/VIQR, flags 7/5/0 và Backspace.
Các test output mô phỏng client áp dụng action, kiểm tra suffix Unicode, undo,
focus, capabilities, shortcut, cấu hình chia sẻ và cô lập context. Test IPC gồm
kiểm tra command/validation và bài test listener/reload/shutdown chỉ chạy trên Unix.
`tests/ffi_smoke.py` gọi thư viện động thật, kiểm tra lifetime chuỗi và đúng 5 exports.

Tái sinh fixture: clone Go vào `tests/upstream/bamboo-core`, checkout commit đã pin;
trong clone chạy `go build -o ../../tools/oracle.exe ../../tools/oracle.go`, rồi từ
gốc repo chạy `python tests/tools/make_fixtures.py`. Go chỉ dùng để tạo fixture.

Nguồn frontend tham khảo, chỉ dùng để hiểu hành vi/API, không sao chép triển khai GPL:

- ibus-bamboo: `49e03d7a36b8a8b796950c158b545af8cf9707d4`.
- [fcitx5-lotus](https://github.com/LotusInputMethod/fcitx5-lotus),
  commit `2cd42cf40f5e270370c31e000cb9f044b0e8ce3d`, `lotus-engine.cpp`,
  `lotus-state.cpp`, `lotus-mode.h` (mode và lifecycle).
- [Fcitx InputMethodEngineV2](https://github.com/fcitx/fcitx5/blob/master/src/lib/fcitx/inputmethodengine.h)
  và [IBusEngine](https://github.com/ibus/ibus/blob/main/src/ibusengine.h) (API adapter).

Chưa benchmark hiệu năng hoặc port encoder/charset legacy. Tập test không chứng
minh tương đương Go cho mọi chuỗi Unicode/DSL và không thay thế kiểm thử desktop Linux.
