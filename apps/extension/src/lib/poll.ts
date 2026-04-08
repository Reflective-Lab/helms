export function createPoller(fn: () => Promise<void>, intervalMs = 3000) {
	let timer: ReturnType<typeof setInterval> | null = null
	let lastUpdate = Date.now()

	function start() {
		if (timer) return
		fn()
		timer = setInterval(async () => {
			await fn()
			lastUpdate = Date.now()
		}, intervalMs)
	}

	function stop() {
		if (timer) {
			clearInterval(timer)
			timer = null
		}
	}

	function secondsSinceUpdate(): number {
		return Math.floor((Date.now() - lastUpdate) / 1000)
	}

	return { start, stop, secondsSinceUpdate }
}
