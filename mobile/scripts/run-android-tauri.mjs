import { spawnSync } from 'node:child_process'
import { existsSync } from 'node:fs'
import { homedir } from 'node:os'
import { join } from 'node:path'

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

const binary = join(process.cwd(), 'node_modules', '.bin', process.platform === 'win32' ? 'tauri.cmd' : 'tauri')
const child = spawnSync(binary, ['android', ...process.argv.slice(2)], { env: environment, stdio: 'inherit' })
process.exit(child.status ?? 1)
