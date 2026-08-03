# file-cli

เครื่องมือ CLI สำหรับจัดการไฟล์และ registry บน Termux / Linux / macOS / Windows

## ติดตั้ง

```bash
cargo install --path .
```

หรือดาวน์โหลด release จาก GitHub:
```
https://github.com/user/file-cli/releases
```

## คำสั่งหลัก

| คำสั่ง | คำอธิบาย |
|--------|----------|
| `dev new --name <name> --type <type> --stack <stack>` | สร้าง item ใหม่ใน registry |
| `dev add --name <name> --part <folder>` | เพิ่ม subfolder ให้ item ที่มีอยู่ |
| `dev set --name <name> --type <type> --stack <stack>` | แก้ไข item |
| `dev list` | แสดงรายการทั้งหมด |
| `dev find <query>` | ค้นหาใน registry |
| `dev glob <pattern> [--path <dir>]` | ค้นหาไฟล์ตาม glob pattern (`*`, `?`) |
| `dev grep <pattern> [--path <dir>]` | ค้นหาเนื้อหาในไฟล์ (ใช้ `rg` ถ้ามี, fallback เป็น pure Rust) |
| `dev size [--top <n>]` | แสดงไฟล์ขนาดใหญ่สุด |
| `dev dedup` | สแกนไฟล์ซ้ำ (แสดงผล) |
| `dev duplicate` | สแกนและลบไฟล์ซ้ำ |
| `dev archive --name <name> --format <zip\|tar\|targz\|7z>` | บีบอัด item เป็น archive |
| `dev automove [--path <dir>]` | ย้ายไฟล์ตามนามสกุลไปยังโฟลเดอร์ที่กำหนด |
| `dev lasted [--top <n>] [--hours <h>]` | แสดงไฟล์ที่แก้ไขล่าสุด |
| `dev sort --by <name\|size\|date> [--desc]` | เรียงลำดับไฟล์ |
| `dev filter --ext <ext> --min-size <size> --max-size <size>` | กรองไฟล์ตามเกณฑ์ |
| `dev ignore --list\|--add <pattern>\|--remove <pattern>` | จัดการ ignore patterns |
| `dev bookmark --list\|--add <path>\|--remove <path>` | จัดการ bookmarks |
| `dev wizard` | ตั้งค่าเริ่มต้น (workspace, templates) |
| `dev vendor --search <name>\|--store <name> <url>` | จัดการ vendor tools |
| `dev doctor` | ตรวจสอบสุขภาพระบบ |
| `dev benchmark` | ทดสอบประสิทธิภาพ |
| `dev templates` | แสดง templates ที่มี |

## คุณสมบัติเด่น

- **Regex pattern matching** — รองรับ glob (`*`, `?`) และค้นหาเนื้อหาด้วย regex ผ่าน `rg`
- **Reject patterns** — ข้ามไฟล์ junk อัตโนมัติ (`.DS_Store`, `Thumbs.db`, `desktop.ini`, `._`, `.Spotlight-V100`, `.Trashes`)
- **Performance** — ใช้ `walkdir` สำหรับ traversal เร็ว, pure Rust fallback เมื่อไม่มี `rg`
- **Vendor bin fallback** — พยายามใช้ system tool ก่อน (zip, tar, 7z, rg) → auto-install → Rust crate fallback → แจ้ง clear message
- **Workspace auto-detect** — ตรวจจับโฟลเดอร์ทำงานอัตโนมัติ (`.file-cli` ใน home directory)

## ตัวอย่าง

```bash
# สร้าง project ใหม่
dev new --name myapp --type asset --stack rust

# ค้นหาไฟล์ TypeScript ทั้งหมด
dev glob "*.ts" --path ~/projects

# ค้นหาคำว่า "TODO" ในโค้ด
dev grep "TODO" --path ~/projects

# บีบอัด project เป็น zip
dev archive --name myapp --format zip

# ย้ายไฟล์ตามนามสกุลอัตโนมัติ
dev automove --path ~/Downloads

# สแกนไฟล์ซ้ำ
dev dedup

# เพิ่ม ignore pattern
dev ignore --add node_modules
dev ignore --add dist
```

## ระบบ Ignore Patterns

Built-in (ข้ามทุกครั้ง): `node_modules`, `.git`, `target`, `.next`, `dist`, `build`, `.turbo`, `.vercel`, `__pycache__`, `.venv`, `.mypy_cache`, `.pytest_cache`, `vendor`, `.cache`, `.idea`, `.vscode`

เพิ่มได้ผ่าน `dev ignore --add <pattern>` — บันทึกไว้ที่ `.file-cli/ignore`

## License

MIT
