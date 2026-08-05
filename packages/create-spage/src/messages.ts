import { green, cyan, red, yellow, bold, dim } from 'kolorist';
import type { UserInput } from './prompts.js';

export function printSuccess(input: UserInput): void {
  console.log();
  console.log(`  ${bold(green('✔'))} ${bold('Project created successfully!')}`);
  console.log();
  console.log(`  ${dim('Next steps:')}`);
  console.log();
  console.log(`  ${cyan('cd')} ${input.name}`);
  console.log(`  ${cyan(`${input.packageManager} run dev`)}`);
  console.log();
  console.log(`  ${dim('Build for production:')}`);
  console.log(`  ${cyan(`${input.packageManager} run build`)}`);
  console.log();
  console.log(`  ${dim('Update Spage resources:')}`);
  console.log(`  ${cyan(`${input.packageManager} run update`)}`);
  console.log();
  console.log(`  ${dim('Happy blogging! 🎉')}`);
  console.log();
}

export function printErrorDirectoryExists(name: string): void {
  console.error();
  console.error(`  ${red('✖')} ${bold('Error')}: Directory "${yellow(name)}" already exists.`);
  console.error(`    Please choose another name or delete the directory and try again.`);
  console.error();
}

export function printErrorCopyFailed(): void {
  console.error();
  console.error(`  ${red('✖')} ${bold('Error')}: Failed to copy template files.`);
  console.error(`    Cleaned up temporary files. Please check disk space and permissions, then try again.`);
  console.error();
}

export function printErrorInstallFailed(name: string, pm: string): void {
  console.error();
  console.error(`  ${yellow('⚠')} ${bold('Warning')}: Dependency installation failed.`);
  console.error(`    Your project was created, but you need to install dependencies manually:`);
  console.error();
  console.error(`    ${cyan('cd')} ${name}`);
  console.error(`    ${cyan(`${pm} install`)}`);
  console.error();
}

export function printCancelled(): void {
  console.log();
  console.log(`  ${yellow('✖')} Initialization cancelled. Cleaned up temporary files.`);
  console.log();
}

export function printBanner(): void {
  console.log();
  console.log(`  ${bold(cyan('create-spage'))} ${dim('- Scaffold a new spage project')}`);
  console.log();
}
