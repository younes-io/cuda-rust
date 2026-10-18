/*
 * SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
 * SPDX-License-Identifier: Apache-2.0
 */

use std::path::Path;

use super::*;

// =============================================================================
// Clean command
// =============================================================================

pub fn clean(ctx: &Context) {
    match clean_context(ctx) {
        Ok(summary) if summary.removed_directories == 0 && summary.removed_files == 0 => {
            println!("Nothing to clean.");
        }
        Ok(summary) => {
            println!(
                "Removed {} directories and {} generated artifacts.",
                summary.removed_directories, summary.removed_files
            );
        }
        Err(error) => {
            eprintln!("Error: {error}");
            std::process::exit(1);
        }
    }
}

#[derive(Debug, Default, Eq, PartialEq)]
pub(super) struct CleanSummary {
    pub(super) removed_directories: usize,
    pub(super) removed_files: usize,
}

pub(super) fn clean_context(ctx: &Context) -> Result<CleanSummary, String> {
    let mut summary = CleanSummary::default();

    match &ctx.workspace {
        Some(workspace) => clean_workspace(&ctx.workspace_root, workspace, &mut summary)?,
        None => clean_standalone_project(&ctx.workspace_root, &mut summary)?,
    }

    Ok(summary)
}

fn clean_standalone_project(project_dir: &Path, summary: &mut CleanSummary) -> Result<(), String> {
    let manifest_path = project_dir.join("Cargo.toml");
    let package_name = package_name_from_manifest(&manifest_path)?;

    if remove_local_target(project_dir)? {
        summary.removed_directories += 1;
    }

    summary.removed_files += remove_generated_artifacts(project_dir, &package_name)?;

    Ok(())
}

fn clean_workspace(
    workspace_root: &Path,
    workspace: &Workspace,
    summary: &mut CleanSummary,
) -> Result<(), String> {
    if remove_local_target(workspace_root)? {
        summary.removed_directories += 1;
    }

    if remove_local_target(&workspace.codegen_crate)? {
        summary.removed_directories += 1;
    }

    let entries = std::fs::read_dir(&workspace.examples_dir).map_err(|error| {
        format!(
            "could not read examples directory {}: {error}",
            workspace.examples_dir.display()
        )
    })?;

    let mut example_dirs = Vec::new();

    for entry in entries {
        let entry = entry.map_err(|error| {
            format!(
                "could not read an entry in {}: {error}",
                workspace.examples_dir.display()
            )
        })?;

        let file_type = entry.file_type().map_err(|error| {
            format!(
                "could not inspect example entry {}: {error}",
                entry.path().display()
            )
        })?;

        if !file_type.is_dir() {
            continue;
        }

        let example_dir = entry.path();
        if example_dir.join("Cargo.toml").is_file() {
            example_dirs.push(example_dir);
        }
    }

    example_dirs.sort();

    for example_dir in example_dirs {
        clean_example(&example_dir, summary)?;
    }

    Ok(())
}

fn clean_example(example_dir: &Path, summary: &mut CleanSummary) -> Result<(), String> {
    let manifest_path = example_dir.join("Cargo.toml");
    let package_name = package_name_from_manifest(&manifest_path)?;

    if remove_local_target(example_dir)? {
        summary.removed_directories += 1;
    }

    summary.removed_files += remove_generated_artifacts(example_dir, &package_name)?;

    Ok(())
}

fn remove_local_target(project_dir: &Path) -> Result<bool, String> {
    let target_dir = project_dir.join("target");

    let metadata = match std::fs::symlink_metadata(&target_dir) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(false);
        }
        Err(error) => {
            return Err(format!(
                "could not inspect {}: {error}",
                target_dir.display()
            ));
        }
    };

    if metadata.file_type().is_symlink() {
        return Err(format!(
            "refusing to remove symlinked target directory {}",
            target_dir.display()
        ));
    }

    if !metadata.is_dir() {
        return Err(format!(
            "expected {} to be a directory",
            target_dir.display()
        ));
    }

    std::fs::remove_dir_all(&target_dir).map_err(|error| {
        format!(
            "could not remove target directory {}: {error}",
            target_dir.display()
        )
    })?;

    println!("Removed {}", target_dir.display());

    Ok(true)
}

fn remove_generated_artifacts(project_dir: &Path, package_name: &str) -> Result<usize, String> {
    let mut removed = 0;

    for path in generated_artifact_paths(project_dir, package_name) {
        let metadata = match std::fs::symlink_metadata(&path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                continue;
            }
            Err(error) => {
                return Err(format!("could not inspect {}: {error}", path.display()));
            }
        };

        if metadata.file_type().is_symlink() {
            return Err(format!(
                "refusing to remove symlinked generated artifact {}",
                path.display()
            ));
        }

        if !metadata.is_file() {
            return Err(format!(
                "expected generated artifact {} to be a file",
                path.display()
            ));
        }

        std::fs::remove_file(&path)
            .map_err(|error| format!("could not remove {}: {error}", path.display()))?;

        println!("Removed {}", path.display());
        removed += 1;
    }

    Ok(removed)
}
