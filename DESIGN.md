# Switch Extractor Desktop - Design System & UI/UX Specification

## 1. Aesthetic Direction & Point of View
- **Style**: **Nintendo Switch OLED Precision Console** — A sleek, dark aerospace console aesthetic inspired by Nintendo Switch hardware, developer debug menus, and modern high-end desktop utilities.
- **Philosophy**: 
  - Pure native feel, zero AI-slop: No generic purple-to-pink gradients, no cards nested inside redundant cards, no bloated whitespace, and no unstyled default system fonts.
  - Tactile, dense, and purposeful: Every pixel, badge, and button has a functional reason to exist.
  - Real-time reactivity: Micro-animations (150–200ms ease-out), smooth progress indicators, real-time log drawer, and immediate visual feedback on file drop or extraction events.

---

## 2. Color Palette & Semantic Tokens

### Dark Mode (Default)
- **Canvas / Background**: `#090C11` (Deep OLED Abyss)
- **Surface Elevation 1 (Sidebar / Panels)**: `#0E131C` (Subtle dark slate)
- **Surface Elevation 2 (Cards / Modals)**: `#141B26` (Elevated console card)
- **Surface Elevation 3 (Inputs / Wells)**: `#1A2230` (Inset input field / terminal well)
- **Border Subtle**: `#222E42` (Precision 1px outline)
- **Border Focus / Active**: `#FF3B53` (Nintendo Neon Coral) or `#00E5FF` (Electric Cyan)

### Brand & Status Accents
- **Primary Accent**: `#FF3B53` (Nintendo Neon Coral — high contrast, vibrant, signature Switch identity)
- **Primary Hover**: `#FF5C70`
- **Secondary Accent**: `#00E5FF` (Electric Cyan — crypto / telemetry / container highlight)
- **Secondary Hover**: `#33EBFF`
- **Success**: `#10B981` (Verified key, extraction complete)
- **Warning**: `#F59E0B` (Missing title key, large unpack warning)
- **Error**: `#EF4444` (Corrupted partition, decryption failure)

### Typography Colors
- **Text Main**: `#F0F4F8` (Crisp off-white, 95% opacity)
- **Text Muted**: `#8B9BB4` (Secondary labels, file sizes, metadata tags)
- **Text Subtle**: `#53637D` (Disabled states, subtle borders, placeholders)

---

## 3. Typography Scale & Font Pairing
1. **Display & Headers**: `Space Grotesk` (Geometric, technical, distinctive console engineering typography)
   - `h1`: 22px / 28px, Bold (700)
   - `h2`: 16px / 22px, SemiBold (600)
   - `h3`: 14px / 20px, Medium (500)
2. **Body & UI**: `Plus Jakarta Sans` (Modern humanist sans-serif with superb legibility at small sizes)
   - `body`: 13px / 18px, Regular (400)
   - `body-sm`: 12px / 16px, Regular (400) & Medium (500)
   - `caption`: 11px / 14px, Medium (500)
3. **Monospace (Hex, Offsets, Hashes, Logs)**: `JetBrains Mono`
   - `code`: 12px / 16px, Regular & Medium (500)

*All fonts are self-hosted and bundled locally within the binary for zero external network dependency.*

---

## 4. Spacing, Radii & Shadows
- **Base Grid**: 4px base (`4px`, `8px`, `12px`, `16px`, `20px`, `24px`, `32px`)
- **Corner Radii**:
  - `sm`: 4px (Badges, tags, tiny buttons)
  - `md`: 8px (Inputs, buttons, small cards)
  - `lg`: 12px (Cards, modal dialogs, hero container)
  - `full`: 9999px (Pills, status dots)
- **Elevation Shadows**:
  - `card`: `0 2px 8px rgba(0, 0, 0, 0.4)`
  - `modal`: `0 12px 32px rgba(0, 0, 0, 0.7)`
  - `glow-accent`: `0 0 16px rgba(255, 59, 83, 0.25)`

---

## 5. Screen Layout & Navigation
- **Window Anatomy**:
  - **Custom Titlebar (36px)**: Windows native drag region, app title with icon, emulator keys status pill (`231 Keys Loaded`), minimize, maximize, close controls.
  - **Sidebar (200px)**: Clean vertical navigation tab bar:
    1. **Extract**: Single game inspection, hero cover card, extraction targets (RomFS, ExeFS, NCAs, Metadata), live extraction progress bar.
    2. **Batch**: Multi-file queue for bulk unpacking directory scans with batch progress.
    3. **Convert**: Direct on-the-fly GameCard XCI ➔ eShop NSP repack converter.
    4. **Key Management**: Emulator keys locator (`prod.keys`, `title.keys`), loaded key inspection table, manual override picker.
    5. **Settings**: Default extraction paths, overwrite behavior, sound/notification toggles.
  - **Main Content Stage**: High-efficiency tabbed view with smooth cross-fade.
  - **Collapsible Log Terminal (Drawer)**: Real-time event log with color-coded tags (`[INFO]`, `[SUCCESS]`, `[WARN]`, `[ERROR]`), autoscroll, and one-click copy.

---

## 6. Micro-Interactions & Performance
- **Micro-Animations**: All transitions limited to 150ms–200ms `cubic-bezier(0.16, 1, 0.3, 1)`.
- **Drag & Drop**: Dropping any `.xci`, `.nsp`, `.nsz`, `.xcz`, `.nca` anywhere onto the window triggers an instant inspect sequence without freezing the UI.
- **Background Threading**: All file I/O, decompression, and decryption run in isolated Tokio threads via Tauri channels. UI thread stays strictly at 60 FPS.

