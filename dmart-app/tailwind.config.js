/** @type {import('tailwindcss').Config} */
module.exports = {
  darkMode: 'class',
  content: [
    "*.html",
    "./src/**/*.rs"
  ],
  theme: {
    extend: {
      fontFamily: {
        sans: ['Inter', 'system-ui', 'sans-serif'],
        mono: ['JetBrains Mono', 'monospace'],
      },
      colors: {
        uci: {
          bg:       'var(--uci-bg)',
          surface:  'var(--uci-surface)',
          card:     'var(--uci-card)',
          border:   'var(--uci-border)',
          accent:   'var(--uci-accent)',
          accent2:  'var(--uci-accent2)',
          low:      'var(--uci-low)',
          moderate: 'var(--uci-moderate)',
          severe:   'var(--uci-severe)',
          critical: 'var(--uci-critical)',
          text:     'var(--uci-text)',
          muted:    'var(--uci-muted)',
        }
      },
      animation: {
        'pulse-slow': 'pulse 3s cubic-bezier(0.4, 0, 0.6, 1) infinite',
        'glow': 'glow 2s ease-in-out infinite alternate',
        'slide-in': 'slideIn 0.3s ease-out',
        'fade-in': 'fadeIn 0.4s ease-out',
      },
      keyframes: {
        glow: {
          '0%': { 'box-shadow': '0 0 5px rgba(59, 130, 246, 0.3)' },
          '100%': { 'box-shadow': '0 0 20px rgba(59, 130, 246, 0.7)' },
        },
        slideIn: {
          '0%': { opacity: '0', transform: 'translateY(-10px)' },
          '100%': { opacity: '1', transform: 'translateY(0)' },
        },
        fadeIn: {
          '0%': { opacity: '0' },
          '100%': { opacity: '1' },
        }
      }
    }
  },
  plugins: [],
}
