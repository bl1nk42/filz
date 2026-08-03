# filz

เครื่องมือ CLI สำหรับจัดการ workspace บน Termux / Linux / macOS / Windows

## ติดตั้ง

### ถ้ามี Rust/Cargo

```bash
cargo install --path .
```

หรือถ้าต้องการ build แบบ release:

```bash
cargo build --release
cp target/release/filz /usr/local/bin/
```

### ถ้าไม่มี Cargo

คุณสามารถใช้ binary ที่คอมไพล์ไว้แล้วจาก GitHub Releases ถ้ามี โดยดาวน์โหลดไฟล์ `filz` (หรือ `filz.exe` บน Windows) แล้ววางไว้ใน `PATH` ของคุณ

ถ้าไม่มี binary release ให้ติดตั้ง Rust ตามขั้นตอนที่ https://rustup.rs แล้วใช้คำสั่งด้านบน

## คำสั่งหลัก

| คำสั่ง | คำอธิบาย |
|--------|----------|
| `filz new` | สร้างรายการใหม่แบบ wizard (step-by-step) |
| `filz add --name <name> --path <path>` | เพิ่มรายการหรือ path ที่ต้องเฝ้าดูให้ filz จัดการต่อ |
| `filz add --name <name> --part <folder>` | เพิ่ม subfolder ให้ item ที่อยู่ใน registry |
| `filz set --name <name> --type <type>` | ปรับ scope ของ item ที่ filz จะเฝ้าดู |
| `filz set --name <name> --path <path>` | เปลี่ยน path ที่ item จะถูกเฝ้าดู |
| `filz list` | แสดงรายการทั้งหมด (ซ่อน archive โดยอัตโนมัติ) |
| `filz find <query>` | ค้นหารายการใน registry |
| `filz glob <pattern> [--path <dir>]` | ค้นหาไฟล์ตามชื่อ (ใช้ * เป็นตัวแทน) |
| `filz grep <pattern> [--path <dir>]` | ค้นหาข้อความในเนื้อหาไฟล์ |
| `filz size [--top <n>]` | แสดงไฟล์ขนาดใหญ่สุด |
| `filz dedup` | สแกนไฟล์ซ้ำ (preview ก่อนเสมอ) |
| `filz duplicate` | สแกนและจัดการไฟล์ซ้ำ (preview ก่อนเสมอ) |
| `filz archive --name <name> --format <zip\|tar\|targz>` | บีบอัด item เป็น archive (preview ก่อนเสมอ) |
| `filz clean` | ล้างไฟล์ขยะ (preview ก่อนเสมอ) |
| `filz vendor --search <name>` | หาเครื่องมือ (ค้นหาใน vendor/, PATH, apt) |
| `filz doctor` | ตรวจสอบสุขภาพระบบ |
| `filz templates` | แสดง templates ที่มี |
| `filz completion <shell>` | ตั้ง auto-complete (bash, fish, zsh, powershell) |

## คุณสมบัติเด่น

- **Wizard สำหรับสร้างรายการ** — `filz new` เป็น step-by-step wizard: เลือกประเภท → เลือก template → ดู path ที่จะสร้าง → ยืนยัน
- **Preview ก่อนเสมอ** — คำสั่งทำลายล้างทั้งหมด (clean, duplicate, archive) จะแสดง preview ก่อนและต้องยืนยัน
- **ค้นหาไฟล์ vs ค้นหาข้อความ** — `glob` ค้นหาไฟล์ตามชื่อ, `grep` ค้นหาข้อความในเนื้อหาไฟล์
- **ซ่อน archive โดยอัตโนมัติ** — `filz list` ซ่อน archive โดยค่าเริ่มต้น ใช้ `--all` เพื่อดูทั้งหมด
- **แสดงผลเรียบง่าย** — ซ่อน stack และ category_path จากผลลัพธ์ปกติ ใช้ `--verbose` เพื่อดูรายละเอียด
- **Completion** — รองรับ Bash/Fish/Zsh/PowerShell ลดการจำ syntax ได้จริง
- **Reject patterns** — ข้ามไฟล์ junk อัตโนมัติ (`.DS_Store`, `Thumbs.db`, `desktop.ini`, `._`, `.Spotlight-V100`, `.Trashes`)
- **Performance** — ใช้ `walkdir` สำหรับ traversal เร็ว, pure Rust fallback เมื่อไม่มี `rg`
- **Vendor bin fallback** — พยายามใช้ system tool ก่อน (zip, tar, 7z, rg) → auto-install → Rust crate fallback → แจ้ง clear message
- **Workspace auto-detect** — ตรวจจับโฟลเดอร์ทำงานอัตโนมัติ (`.file-cli` ใน home directory)

## ตัวอย่าง

```bash
# สร้าง project ใหม่แบบ wizard (step-by-step)
filz new

# สร้าง project ใหม่ด้วย template โดยไม่ต้องผ่าน wizard
filz new --name myapp --type work --template website

# สร้าง project แบบไม่ต้องยืนยัน
filz new --name myapp --type work --template website --yes

# สร้าง project ที่มีโครงสร้างลึก 10 ชั้น
filz new --name deep-app --type work --template complex --yes

# สร้าง Python service พร้อม virtualenv และ docs
filz new --name api-service --type work --template python-service --yes

# แสดงเทมเพลตที่มี
filz templates

# ค้นหาไฟล์ TypeScript ทั้งหมด
filz glob "*.ts" --path ~/projects

# ค้นหาคำว่า "TODO" ในโค้ด
filz grep "TODO" --path ~/projects

# บีบอัด project เป็น zip (แสดง preview ก่อน)
filz archive --name myapp --format zip

# สแกนไฟล์ซ้ำ (แสดง preview เท่านั้น)
filz dedup

# สแกนและลบไฟล์ซ้ำ (ต้องยืนยัน)
filz dedup --yes

# เพิ่ม ignore pattern
filz ignore --add node_modules
filz ignore --add dist

# ตั้ง auto-complete ใน shell
filz --completion bash
```

## เทมเพลตที่พร้อมใช้งาน

ปัจจุบันรองรับ template หลายรูปแบบ ได้แก่:

- `app` — โครงงาน Rust เบื้องต้น
- `archive` — โฟลเดอร์ archive สำหรับเก็บไฟล์
- `benchmark` — สคริปต์ benchmark เปรียบเทียบ `rg` และ `find|grep`
- `blog` — เว็บไซต์บล็อก
- `complex` — โครงสร้างโปรเจคลึก 10 ชั้น พร้อม token replacement
- `note` — โน้ตง่าย ๆ
- `photo` — โปรเจคจัดการภาพ
- `python-service` — service Python พร้อมโฟลเดอร์ย่อยและ docs
- `website` — เว็บไซต์ frontend เบื้องต้น

ใช้ `filz templates` เพื่อดูรายการเทมเพลตทั้งหมดและรายละเอียดของแต่ละอัน

## การใช้งาน wizard สำหรับ `filz new`

เมื่อรัน `filz new` โดยไม่ระบุพารามิเตอร์ จะแสดง wizard แบบ step-by-step:

1. **ขั้นตอนที่ 1**: เลือกประเภทงาน (work, doc, asset, archive, tool)
2. **ขั้นตอนที่ 2**: เลือก template (website, app, blog, note, photo) หรือ stack (rust, node, python, go)
3. **ขั้นตอนที่ 3**: ตั้งชื่อรายการ
4. **ขั้นตอนที่ 4**: ดูสรุปและยืนยัน

สามารถข้ามขั้นตอนได้โดยใช้ flags:
- `--name <name>` — ข้ามการตั้งชื่อ
- `--type <type>` — ข้ามการเลือกประเภท
- `--template <template>` — ข้ามการเลือก template/stack
- `--yes` — ข้ามการยืนยัน

## ระบบ Ignore Patterns

Built-in (ข้ามทุกครั้ง): `node_modules`, `.git`, `target`, `.next`, `dist`, `build`, `.turbo`, `.vercel`, `__pycache__`, `.venv`, `.mypy_cache`, `.pytest_cache`, `vendor`, `.cache`, `.idea`, `.vscode`

เพิ่มได้ผ่าน `filz ignore --add <pattern>` — บันทึกไว้ที่ `.file-cli/ignore`

## Completion

รองรับ auto-complete สำหรับ Bash, Fish, Zsh, และ PowerShell:

```bash
# ติดตั้งสำหรับ Bash
filz --completion bash

# ติดตั้งสำหรับ Zsh
filz --completion zsh

# ติดตั้งสำหรับ Fish
filz --completion fish

# ติดตั้งสำหรับ PowerShell
filz --completion powershell
```

## License

MIT