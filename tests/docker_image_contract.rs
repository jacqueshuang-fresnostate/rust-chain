const DOCKERFILE: &str = include_str!("../Dockerfile");
const DOCKERIGNORE: &str = include_str!("../.dockerignore");
const DOCKER_IMAGE_WORKFLOW: &str = include_str!("../.github/workflows/docker-image.yml");
const STANDARD_COMPOSE: &str = include_str!("../docker-compose.example.yml");
const ONEPANEL_COMPOSE: &str = include_str!("../docker-compose.1panel.example.yml");
const LOCAL_COMPOSE: &str = include_str!("../docker-compose.yml");

const SAME_ORIGIN_FLAG: &str = "VITE_API_SAME_ORIGIN";
const API_BASE_URL: &str = "VITE_API_BASE_URL";

fn docker_instructions(source: &str) -> Vec<String> {
    let mut instructions = Vec::new();
    let mut current = String::new();

    for raw_line in source.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        let continues = line.ends_with('\\');
        let fragment = line.trim_end_matches('\\').trim_end();
        if !current.is_empty() {
            current.push(' ');
        }
        current.push_str(fragment);

        if !continues {
            instructions.push(std::mem::take(&mut current));
        }
    }

    if !current.is_empty() {
        instructions.push(current);
    }
    instructions
}

fn is_named_stage(instruction: &str, expected_name: &str) -> bool {
    let words: Vec<_> = instruction.split_whitespace().collect();
    words
        .windows(2)
        .any(|pair| pair[0].eq_ignore_ascii_case("AS") && pair[1] == expected_name)
}

fn instruction_body<'a>(instruction: &'a str, expected_opcode: &str) -> Option<&'a str> {
    let opcode_end = instruction.find(char::is_whitespace)?;
    if !instruction[..opcode_end].eq_ignore_ascii_case(expected_opcode) {
        return None;
    }
    Some(instruction[opcode_end..].trim_start())
}

fn docker_arg<'a>(instruction: &'a str, expected_name: &str) -> Option<Option<&'a str>> {
    let body = instruction_body(instruction, "ARG")?;
    let (name, default) = body
        .split_once('=')
        .map_or((body, None), |(name, default)| (name, Some(default)));
    (name.trim() == expected_name).then(|| default.map(str::trim))
}

fn shell_assignment(token: &str) -> Option<(&str, &str)> {
    let token = token
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
        .unwrap_or(token);
    let (name, value) = token.split_once('=')?;
    Some((name, value.trim_matches('"')))
}

fn is_same_origin_arg_reference(value: &str) -> bool {
    value == "$VITE_API_SAME_ORIGIN" || value == "${VITE_API_SAME_ORIGIN}"
}

fn run_scopes_same_origin_to_npm_build(instruction: &str) -> bool {
    let Some(command) = instruction_body(instruction, "RUN") else {
        return false;
    };
    let tokens: Vec<_> = command.split_whitespace().collect();

    tokens.windows(3).enumerate().any(|(npm_index, window)| {
        if window != ["npm", "run", "build"] {
            return false;
        }

        tokens[..npm_index]
            .iter()
            .rev()
            .map_while(|token| shell_assignment(token))
            .any(|(name, value)| name == SAME_ORIGIN_FLAG && is_same_origin_arg_reference(value))
    })
}

fn active_config_lines(source: &str) -> impl Iterator<Item = &str> {
    source.lines().filter_map(|line| {
        let active = line.split('#').next()?.trim();
        (!active.is_empty()).then_some(active)
    })
}

fn config_sets_variable(source: &str, variable: &str) -> bool {
    active_config_lines(source).any(|line| {
        let candidate = line.strip_prefix('-').unwrap_or(line).trim_start();
        candidate.strip_prefix(variable).is_some_and(|suffix| {
            suffix.is_empty() || suffix.starts_with(':') || suffix.starts_with('=')
        })
    })
}

fn dockerignore_excludes_admin_env_files(source: &str) -> bool {
    let patterns: Vec<_> = active_config_lines(source).collect();
    if patterns
        .iter()
        .any(|pattern| pattern.starts_with('!') && pattern.contains(".env"))
    {
        return false;
    }

    let excludes_env = patterns
        .iter()
        .any(|pattern| matches!(*pattern, ".env" | "**/.env" | "web/.env"));
    let excludes_env_variants = patterns.iter().any(|pattern| {
        matches!(
            *pattern,
            ".env.*" | ".env*" | "**/.env.*" | "**/.env*" | "web/.env.*" | "web/.env*"
        )
    });
    excludes_env && excludes_env_variants
}

fn workflow_has_unsafe_vite_override(source: &str) -> bool {
    active_config_lines(source).any(|line| {
        if line.contains(API_BASE_URL) {
            return true;
        }
        if !line.contains(SAME_ORIGIN_FLAG) {
            return false;
        }

        let compact: String = line
            .chars()
            .filter(|character| !character.is_whitespace())
            .collect();
        let Some((_, value)) = compact.split_once("VITE_API_SAME_ORIGIN=") else {
            return true;
        };
        value.trim_matches(['\'', '"', ',', ']']) != "true"
    })
}

fn stage_instructions(source: &str, stage_name: &str) -> Option<Vec<String>> {
    let instructions = docker_instructions(source);
    let start = instructions
        .iter()
        .position(|instruction| is_named_stage(instruction, stage_name))?;
    let end = instructions[start + 1..]
        .iter()
        .position(|instruction| instruction.to_ascii_uppercase().starts_with("FROM "))
        .map_or(instructions.len(), |offset| start + 1 + offset);
    Some(instructions[start..end].to_vec())
}

fn validate_admin_same_origin_build_contract(
    dockerfile: &str,
    dockerignore: &str,
    docker_workflow: &str,
    compose_sources: &[(&str, &str)],
) -> Vec<String> {
    let mut errors = Vec::new();
    let Some(web_builder) = stage_instructions(dockerfile, "web-builder") else {
        return vec!["Dockerfile must define the web-builder stage".to_owned()];
    };

    let build_steps: Vec<_> = web_builder
        .iter()
        .enumerate()
        .filter(|(_, instruction)| {
            instruction_body(instruction, "RUN").is_some()
                && instruction
                    .split_whitespace()
                    .collect::<Vec<_>>()
                    .windows(3)
                    .any(|window| window == ["npm", "run", "build"])
        })
        .collect();
    if build_steps.len() != 1 {
        errors.push("web-builder must contain exactly one npm run build step".to_owned());
    }

    let same_origin_args: Vec<_> = web_builder
        .iter()
        .enumerate()
        .filter_map(|(index, instruction)| {
            docker_arg(instruction, SAME_ORIGIN_FLAG).map(|default| (index, default))
        })
        .collect();
    if same_origin_args.len() != 1 || same_origin_args[0].1 != Some("true") {
        errors.push("web-builder must default VITE_API_SAME_ORIGIN to true with ARG".to_owned());
    }

    if let Some((build_index, build_step)) = build_steps.first() {
        if !run_scopes_same_origin_to_npm_build(build_step) {
            errors.push(
                "VITE_API_SAME_ORIGIN must be injected in the npm run build command scope"
                    .to_owned(),
            );
        }
        if same_origin_args
            .first()
            .is_some_and(|(arg_index, _)| arg_index >= build_index)
        {
            errors.push("the same-origin build ARG must precede npm run build".to_owned());
        }
    }

    if web_builder
        .iter()
        .any(|instruction| instruction.contains("VITE_API_BASE_URL"))
    {
        errors
            .push("the integrated same-origin image must not inject VITE_API_BASE_URL".to_owned());
    }

    for variable in [SAME_ORIGIN_FLAG, API_BASE_URL] {
        if docker_instructions(dockerfile).iter().any(|instruction| {
            instruction_body(instruction, "ENV").is_some() && instruction.contains(variable)
        }) {
            errors.push(format!("{variable} must not be persisted with Docker ENV"));
        }
    }

    if !dockerignore_excludes_admin_env_files(dockerignore) {
        errors.push(
            ".dockerignore must exclude web/.env and web/.env.* from image builds".to_owned(),
        );
    }

    if workflow_has_unsafe_vite_override(docker_workflow) {
        errors.push(
            "the Docker image workflow must use the same-origin default and omit VITE_API_BASE_URL"
                .to_owned(),
        );
    }

    for (name, source) in compose_sources {
        for variable in [SAME_ORIGIN_FLAG, API_BASE_URL] {
            if config_sets_variable(source, variable) {
                errors.push(format!(
                    "{name} must not set build-time {variable} at container runtime"
                ));
            }
        }
    }

    errors
}

#[test]
fn integrated_image_injects_admin_same_origin_only_at_build_time() {
    let errors = validate_admin_same_origin_build_contract(
        DOCKERFILE,
        DOCKERIGNORE,
        DOCKER_IMAGE_WORKFLOW,
        &[
            ("docker-compose.example.yml", STANDARD_COMPOSE),
            ("docker-compose.1panel.example.yml", ONEPANEL_COMPOSE),
            ("docker-compose.yml", LOCAL_COMPOSE),
        ],
    );

    assert!(errors.is_empty(), "{}", errors.join("\n"));
}

#[test]
fn dockerfile_does_not_depend_on_remote_frontend() {
    assert!(DOCKERFILE.lines().all(|line| {
        !line
            .trim_start()
            .to_ascii_lowercase()
            .starts_with("# syntax=")
    }));
    assert!(
        DOCKERFILE.contains("RUN --mount=type=cache"),
        "the Dockerfile must retain BuildKit cache mounts"
    );
    assert!(
        DOCKERFILE.contains("COPY --chmod="),
        "the Dockerfile must retain BuildKit copy-permission support"
    );
}

#[test]
fn legacy_unscoped_admin_build_is_rejected() {
    let legacy_dockerfile = r#"
FROM node:24-bookworm-slim AS web-builder
WORKDIR /workspace/web
RUN npm run build

FROM debian:bookworm-slim AS runtime
COPY --from=web-builder /workspace/web/dist /usr/share/nginx/html
"#;

    let errors = validate_admin_same_origin_build_contract(
        legacy_dockerfile,
        ".env\n.env.*\n",
        "context: .\n",
        &[],
    );

    assert!(
        errors
            .iter()
            .any(|error| error.contains("default VITE_API_SAME_ORIGIN"))
    );
    assert!(
        errors
            .iter()
            .any(|error| error.contains("npm run build command scope"))
    );
}

#[test]
fn equivalent_multiline_command_is_accepted_without_persisting_vite_env() {
    let dockerfile = r#"
FROM node:24-bookworm-slim AS web-builder
ARG VITE_API_SAME_ORIGIN=true
WORKDIR /workspace/web
RUN VITE_API_SAME_ORIGIN=$VITE_API_SAME_ORIGIN \
    npm run build

FROM debian:bookworm-slim AS runtime
COPY --from=web-builder /workspace/web/dist /usr/share/nginx/html
"#;

    let errors = validate_admin_same_origin_build_contract(
        dockerfile,
        "**/.env\n**/.env.*\n",
        "context: .\n",
        &[],
    );

    assert!(errors.is_empty(), "{}", errors.join("\n"));
}

#[test]
fn runtime_and_workflow_overrides_are_rejected() {
    let dockerfile = r#"
FROM node:24-bookworm-slim AS web-builder
ARG VITE_API_SAME_ORIGIN=true
RUN VITE_API_SAME_ORIGIN="${VITE_API_SAME_ORIGIN}" npm run build

FROM debian:bookworm-slim AS runtime
ENV VITE_API_BASE_URL=https://api.example.test
"#;
    let errors = validate_admin_same_origin_build_contract(
        dockerfile,
        "**/.env\n**/.env.*\n",
        "build-args: VITE_API_SAME_ORIGIN=false\n",
        &[(
            "compose.yml",
            "services:\n  api:\n    environment:\n      VITE_API_BASE_URL: https://api.example.test\n",
        )],
    );

    assert!(errors.iter().any(|error| error.contains("Docker ENV")));
    assert!(
        errors
            .iter()
            .any(|error| error.contains("Docker image workflow"))
    );
    assert!(errors.iter().any(|error| error.contains("compose.yml")));
}
