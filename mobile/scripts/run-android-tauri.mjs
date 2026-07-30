import { spawnSync } from 'node:child_process'
import { copyFileSync, existsSync, mkdirSync } from 'node:fs'
import { homedir } from 'node:os'
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'

function defaultSdkPath() {
  if (process.platform === 'darwin') return join(homedir(), 'Library', 'Android', 'sdk')
  if (process.platform === 'win32') return join(process.env.LOCALAPPDATA || homedir(), 'Android', 'Sdk')
  return join(homedir(), 'Android', 'Sdk')
}

const sdkPath = process.env.ANDROID_HOME || process.env.ANDROID_SDK_ROOT || defaultSdkPath()
const environment = { ...process.env }
if (existsSync(sdkPath)) {
  environment.ANDROID_HOME ||= sdkPath
  environment.ANDROID_SDK_ROOT ||= sdkPath
}

const scriptDirectory = dirname(fileURLToPath(import.meta.url))
const mainActivitySource = join(scriptDirectory, '..', 'src-tauri', 'android', 'MainActivity.kt')
const mainActivityTarget = join(
  scriptDirectory,
  '..',
  'src-tauri',
  'gen',
  'android',
  'app',
  'src',
  'main',
  'java',
  'com',
  'hippo',
  'exchange',
  'mobile',
  'MainActivity.kt',
)

function syncMainActivity() {
  mkdirSync(dirname(mainActivityTarget), { recursive: true })
  copyFileSync(mainActivitySource, mainActivityTarget)
}

const args = process.argv.slice(2)
const command = args[0]
if (command !== 'init') {
  syncMainActivity()
}

const binary = join(process.cwd(), 'node_modules', '.bin', process.platform === 'win32' ? 'tauri.cmd' : 'tauri')
const child = spawnSync(binary, ['android', ...args], { env: environment, stdio: 'inherit' })

if (command === 'init' && child.status === 0) {
  syncMainActivity()
}

process.exit(child.status ?? 1)
