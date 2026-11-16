/** @type {import('tailwindcss').Config} */
export default {
  content: ['./index.html', './src/**/*.{svelte,ts}'],
  theme: {
    extend: {
      colors: {
        institucional: {
          50: '#eef4fb',
          100: '#d7e4f4',
          500: '#1b4777',
          600: '#153a63',
          700: '#0f2c4c',
          900: '#08192b'
        }
      },
      fontFamily: {
        sans: ['Inter', 'Segoe UI', 'system-ui', 'sans-serif']
      }
    }
  },
  plugins: []
};
