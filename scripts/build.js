import Path from 'path';
import Chalk from 'chalk';
import FileSystem from 'fs';
import { build } from 'vite';
import { fileURLToPath } from 'node:url'
import { dirname } from 'node:path';

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);

function buildRenderer() {
    return build({
        configFile: Path.join(__dirname, '..', 'vite.config.js'),
        base: './',
        mode: 'production'
    });
}

async function copy(path) {
    const source = Path.join(__dirname, '..', 'src', 'main', path);
    const dest = Path.join(__dirname, '..', 'build', 'main', path);
    if (!FileSystem.existsSync(source)) {
        console.error(`Error: Source path does not exist - ${source}`);
        return Promise.reject(new Error('Source path does not exist'));
    }
    
    await FileSystem.promises.mkdir(Path.dirname(dest), { recursive: true });
    await FileSystem.promises.cp(source, dest, { recursive: true });

    console.log(Chalk.yellowBright(`Copied: ${source} → ${dest}`));
}

FileSystem.rmSync(Path.join(__dirname, '..', 'build'), {
    recursive: true,
    force: true,
})

console.log(Chalk.blueBright('Transpiling renderer & main...'));

Promise.allSettled([
    buildRenderer(),
    copy('./')
]).then(() => {
    console.log(Chalk.greenBright('Renderer & main successfully transpiled! (ready to be built with electron-builder)'));
}).catch(err => {
    console.error(Chalk.redBright('Build failed:'), err);
});
