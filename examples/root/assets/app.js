import { greeting } from './greeting.js';

const report = document.getElementById('report');

const responses = await Promise.all(
    ['./assets/styles.css', 'assets/logo.svg', './missing.js'].map(
        async (path) => {
            const response = await fetch(path);
            return `${path} → ${response.status} ${response.headers.get('content-type')}`;
        },
    ),
);

report.textContent = [greeting, ...responses].join('\n');
report.style.whiteSpace = 'pre';
