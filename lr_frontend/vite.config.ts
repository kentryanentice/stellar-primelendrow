import { defineConfig } from 'vite'
import react, { reactCompilerPreset } from '@vitejs/plugin-react'
import babel from '@rolldown/plugin-babel'

// https://vite.dev/config/
export default defineConfig({
  plugins: [
    react(),
    babel({ presets: [reactCompilerPreset()] })
  ],
  server :{
		port: 3000,
    host: true,
		watch: {
		usePolling: true,
		interval: 30,
		}
	},
  build: {
    rolldownOptions: {
      output: {
        codeSplitting: {
          groups: [
            // ONNX Runtime must live in a chunk of its own. With `proxy` on
            // (src/functions/ocr/session.ts) it starts its worker from its own
            // `import.meta.url`; bundled into the KYC chunk, that URL is a file
            // importing the app entry, which touches `document` on load and
            // kills the worker ("no available backend found. ERR: [wasm]
            // [object ErrorEvent]"). Dev never showed it: Vite serves the
            // package as a standalone pre-bundled file there.
            { name: 'onnxruntime', test: /[\\/]node_modules[\\/]onnxruntime-(web|common)[\\/]/, priority: 10 },
          ],
        },
      },
    },
  },
})
