import { svelte } from '@sveltejs/vite-plugin-svelte'
import { resolve } from 'path'
import { defineConfig } from 'vite'

export default defineConfig({
	plugins: [svelte()],
	resolve: {
		alias: {
			$lib: resolve(__dirname, 'src/lib')
		}
	},
	base: './',
	build: {
		outDir: 'dist',
		emptyOutDir: true,
		rollupOptions: {
			input: {
				sidepanel: resolve(__dirname, 'src/sidepanel.html')
			},
			output: {
				entryFileNames: '[name].js',
				chunkFileNames: '[name].js',
				assetFileNames: '[name].[ext]'
			}
		}
	}
})
