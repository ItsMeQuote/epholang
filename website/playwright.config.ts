import { defineConfig } from "@playwright/test";
export default defineConfig({
  testDir: "tests",
  testMatch: "**/*.spec.ts",
  workers: 2,
  use: { baseURL: "http://127.0.0.1:4321", headless: true },
  webServer: {
    command: "npm run preview -- --port 4321",
    url: "http://127.0.0.1:4321/epholang/",
    reuseExistingServer: !process.env.CI,
  },
});
