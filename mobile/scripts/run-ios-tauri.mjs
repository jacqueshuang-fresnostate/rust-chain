import { spawnSync } from 'node:child_process'
import { chmodSync, mkdtempSync, mkdirSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'

const wrapperRoot = mkdtempSync(join(tmpdir(), 'hippo-tauri-git-'))
const gitHome = join(wrapperRoot, 'home')
const gitBin = join(wrapperRoot, 'bin')
const gitWrapper = join(gitBin, 'git')
const tauriArguments = process.argv.slice(2)

mkdirSync(gitHome, { recursive: true })
mkdirSync(gitBin, { recursive: true })

// SwiftPM 会在 bare Git 缓存中读取依赖。仅对子进程 Git 放宽校验，避免影响 Xcode 钥匙串和用户全局 Git 配置。
writeFileSync(join(gitHome, '.gitconfig'), '[safe]\n\tbareRepository = all\n')
writeFileSync(gitWrapper, `#!/bin/sh\nHOME=${JSON.stringify(gitHome)} exec /usr/bin/git "$@"\n`)
chmodSync(gitWrapper, 0o755)

const binary = join(process.cwd(), 'node_modules', '.bin', process.platform === 'win32' ? 'tauri.cmd' : 'tauri')
let exitCode = 1

try {
  if (tauriArguments[0] === 'build') {
    // Tauri 2 重复归档时不会覆盖旧产物，先清理被 Git 忽略的 iOS 构建目录以保证命令可重复执行。
    rmSync(join(process.cwd(), 'src-tauri', 'gen', 'apple', 'build'), { force: true, recursive: true })
  }

  const child = spawnSync(binary, ['ios', ...tauriArguments], {
    env: { ...process.env, PATH: `${gitBin}:${process.env.PATH || ''}` },
    stdio: 'inherit',
  })
  exitCode = child.status ?? 1
} finally {
  rmSync(wrapperRoot, { force: true, recursive: true })
}

process.exit(exitCode)
