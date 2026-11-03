# Quy ước tổ chức tệp

- Tất cả tệp kiểm thử phải nằm trong thư mục `tests/` của repo.
- Gom script hỗ trợ, công cụ đối chiếu, fixture, dữ liệu mẫu và kết quả kiểm thử
  thủ công vào `tests/` hoặc các thư mục con; không tạo rải rác ở thư mục gốc,
  `src/` hay bên ngoài repo.
- Công cụ hỗ trợ kiểm thử đặt trong `tests/tools/`. Khi di chuyển tệp, cập nhật
  đường dẫn trong script và tài liệu liên quan.
- Các sản phẩm build do Cargo quản lý vẫn dùng thư mục chuẩn `target/`.

# Kiến trúc viet-ime

- Core Rust đặt tại `core-rust/`; thuật toán gõ nằm trong `composition.rs`,
  điều phối sự kiện và output nằm trong `engine.rs`.
- Wrapper tại `wrappers/ibus-c/` và `wrappers/fcitx5-cpp/`, mỗi file plugin dưới
  250 dòng. Wrapper chỉ chuyển sự kiện, metadata/capabilities và thực thi action;
  không thêm luật tiếng Việt hay đọc cấu hình người dùng.
- C ABI chỉ có 5 hàm trong `core-rust/include/viet_core.h`. Chuỗi kết quả do engine
  sở hữu; không thêm hàm giải phóng kết quả. Thay đổi ABI phải cập nhật header,
  hai wrapper và kiểm thử cùng lúc.
- Tất cả kiểm thử vẫn tập trung ở `tests/` tại gốc monorepo. Khai báo đường dẫn
  integration test trong `core-rust/Cargo.toml`, không tạo `core-rust/tests/`.
- Cài đặt/đóng gói nằm trong `install.sh` và `packaging/`. Không build bằng sudo,
  không tự ghi đè cấu hình input sources/environment của người dùng.
- `viet-config` dùng lại module `control`/`settings` trong core cho GUI và CLI;
  không tạo bộ parser hoặc logic cấu hình riêng trong wrapper hay frontend.
- Build mặc định trên Windows chỉ tạo thư viện, không tạo `viet-config.exe`.
  Binary `viet-config` dành riêng cho Linux, bật qua feature `config-app` trong
  bộ cài/CMake và quy trình kiểm thử Linux.
- `build.ps1`/`build.sh` mặc định tạo gói binary Linux `.deb` trong `disk/`, dùng
  Zig cross-compiler + Debian SDK trên Windows, Linux native trên Linux.
  Không cài/chạy Linux tạm, VM hay container để build khi làm việc trên Windows.
  Máy đích chỉ cài package, không tự build.
  Chỉ xuất package sau khi kiểm tra payload ELF và dependency. Không đổi đuôi
  source archive thành installer hay tạo DEB giả khi thiếu Linux builder.
- Bản mã nguồn chỉ xuất khi yêu cầu rõ `--source`. Không chép test, cache,
  upstream, `target/`, cấu hình cá nhân hoặc binary Windows vào bản nguồn đó.
