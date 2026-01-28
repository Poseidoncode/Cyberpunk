/** @type {import('tailwindcss').Config} */
export default {
    content: [
        "./index.html",
        "./src/**/*.{vue,js,ts,jsx,tsx}",
    ],
    theme: {
        extend: {
            colors: {
                terminal: {
                    bg: '#0a0a0a',
                    primary: '#33ff00',
                    secondary: '#ffb000',
                    muted: '#1f521f',
                    error: '#ff3333',
                    border: '#1f521f',
                },
            },
            fontFamily: {
                mono: ['"JetBrains Mono"', 'monospace'],
            },
            keyframes: {
                blink: {
                    '0%, 49%': { opacity: '1' },
                    '50%, 100%': { opacity: '0' },
                },
                glitch: {
                    '0%, 100%': { transform: 'translate(0)' },
                    '33%': { transform: 'translate(-2px, 1px)' },
                    '66%': { transform: 'translate(2px, -1px)' },
                },
            },
            animation: {
                blink: 'blink 1s step-end infinite',
                glitch: 'glitch 0.3s ease-in-out',
            },
        },
    },
    plugins: [],
}
