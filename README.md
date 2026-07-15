<div align="center">

# Samoyed Work

[![Windows](https://img.shields.io/badge/platform-Windows-blue?logo=windows)](https://github.com/user-attachments/samoyed-work)
[![Tauri 2](https://img.shields.io/badge/Tauri-2.x-orange?logo=tauri)](https://v2.tauri.app/)
[![React 19](https://img.shields.io/badge/React-19-61dafb?logo=react)](https://react.dev/)
[![Rust](https://img.shields.io/badge/Rust-1.80+-000000?logo=rust)](https://www.rust-lang.org/)
[![License](https://img.shields.io/badge/license-MIT-green)](./LICENSE)

[English](./README.md) | [简体中文](./README_zh.md)

<img src="assets/screenshots/English1.1.0.png" alt="Samoyed Work Screenshot" width="800" />

</div>

## Installation

Download the latest Windows installer from [Releases](https://github.com/light-misty/Samoyed-Work/releases) and run it to install.

## Features

### AI Agent
- Multi-mode agent (Plan / Code / Document modes), autonomous task execution
- SubAgent workflow for complex task decomposition
- LSP (Language Server Protocol) integration with real-time code diagnostics
- Permission system with granular control over file and command operations
- Extensible Skill system for loading custom capabilities
- Multi-turn conversational operations
- Real-time streaming of AI thoughts and results
- Visual workflow timeline showing each step
- Live code execution preview

### Multiple AI Models
- OpenAI-compatible API, Anthropic Claude, Google Gemini, Ollama local models
- Custom API endpoint support
- Health monitoring with auto-recovery
- Real-time token usage tracking

### Workspace Management
- Multiple workspaces mapped to local directories
- File tree browsing and search
- Create, delete, rename files within workspaces
- Auto-detection when directories are deleted
- Git repository status display

### Document Processing
- Word (.docx): read, create, edit, format conversion, structure analysis
- Excel (.xlsx): read, create, edit, data extraction
- PPT (.pptx): read, create, edit, slide extraction
- PDF: text extraction
- Markdown / Plain Text: read and convert
- Python code: sandboxed execution with plotting and data analysis support

### Version History
- Automatic version snapshots on file changes
- Configurable retention policy (by count or days)
- Version history browsing with diff comparison
- One-click rollback

### Session Management
- Switch between multiple sessions
- AI continues running in background after switching
- Auto-generated session titles
- Session list pagination for large history

### Prompt Templates
- Built-in templates for common tasks
- Custom templates with variables
- Category-based organization

### User Experience
- Dark / Light / System theme
- Chinese / English interface
- Global shortcuts (Ctrl+N new session, Ctrl+W close, Ctrl+B sidebar, Ctrl+, settings)
- File attachment upload (images, documents, text)
- Automatic update detection and installation

