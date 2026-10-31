import { defineConfig } from '@playwright/test';
export default defineConfig({
  testDir: './tests/browser',
  workers: 1,
  use: { baseURL: 'http://127.0.0.1:1420', headless: true, viewport: {width: 1280, height: 900}, launchOptions: { executablePath: process.env.CHROMIUM_PATH || '/usr/bin/chromium', args: ['--no-sandbox'] } },
  webServer: [
    {command:'npm run dev -- --host 127.0.0.1',url:'http://127.0.0.1:1420',reuseExistingServer:!process.env.CI},
    {command:'cargo run --manifest-path crates/meepix-app/Cargo.toml --example smoke_bridge -- work/browser-projects',url:'http://127.0.0.1:1421',reuseExistingServer:!process.env.CI,timeout:120000},
  ],
});
