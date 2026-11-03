# Quy ước tổ chức tệp

- Tất cả tệp kiểm thử phải nằm trong thư mục `tests/` của repo.
- Gom script hỗ trợ, công cụ đối chiếu, fixture, dữ liệu mẫu và kết quả kiểm thử
  thủ công vào `tests/` hoặc các thư mục con; không tạo rải rác ở thư mục gốc,
  `src/` hay bên ngoài repo.
- Công cụ hỗ trợ kiểm thử đặt trong `tests/tools/`. Khi di chuyển tệp, cập nhật
  đường dẫn trong script và tài liệu liên quan.
- Các sản phẩm build do Cargo quản lý vẫn dùng thư mục chuẩn `target/`.
