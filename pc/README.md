# Market Exchange App

A high-performance blockchain exchange frontend built with Vue 3, Vite, Tauri v2, and TailwindCSS.

## Project Architecture

- **Frontend Framework**: Vue 3 (Composition API)
- **Build Tool**: Vite
- **Language**: TypeScript
- **Desktop Wrapper**: Tauri v2
- **UI Framework**: Naive UI + TailwindCSS
- **State Management**: Pinia
- **Routing**: Vue Router
- **Data**: Axios + TanStack Query

## Setup & Run

### Prerequisites
- Node.js (v18+)
- Rust (latest stable) for Tauri

### Install Dependencies
```bash
npm install
```

### Run Development
```bash
# Web-only mode
npm run dev

# Desktop mode (Tauri)
npm run tauri dev
```

### Build
```bash
npm run build
npm run tauri build
```
