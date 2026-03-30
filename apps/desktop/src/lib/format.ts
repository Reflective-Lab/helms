export function formatMoney(minor: number, currencyCode: string) {
	return new Intl.NumberFormat('en-US', {
		style: 'currency',
		currency: currencyCode,
		maximumFractionDigits: 0
	}).format(minor / 100)
}

export function formatTime(value?: string) {
	if (!value) return 'n/a'
	return new Intl.DateTimeFormat('en-US', {
		month: 'short',
		day: 'numeric',
		hour: '2-digit',
		minute: '2-digit'
	}).format(new Date(value))
}
