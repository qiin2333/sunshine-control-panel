import Path from 'path';
import Chalk from 'chalk';
import FileSystem from 'fs';
import { build } from 'vite';

function buildRenderer() {
    return build({
        configFile: Path.join(__dirname, '..', 'vite.config.js'),
        base: './',
        mode: 'production'
    });
}

function copy(path) {
    FileSystem.cpSync(
        Path.join(__dirname, '..', 'src', 'main', path),
        Path.join(__dirname, '..', 'build', 'main', path),
        { recursive: true }
    );
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
});
