export function formatTime(iso: string): string {
	const date = new Date(iso)
	const now = new Date()
	const diffMs = now.getTime() - date.getTime()
	const diffSec = Math.floor(diffMs / 1000)

	if (diffSec < 60) return `${diffSec}s ago`
	const diffMin = Math.floor(diffSec / 60)
	if (diffMin < 60) return `${diffMin}m ago`
	const diffHr = Math.floor(diffMin / 60)
	if (diffHr < 24) return `${diffHr}h ago`
	return date.toLocaleDateString()
}

export function formatMoney(minorUnits: number, currency = 'USD'): string {
	return new Intl.NumberFormat('en-US', {
		style: 'currency',
		currency,
		minimumFractionDigits: 2
	}).format(minorUnits / 100)
}
