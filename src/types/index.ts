export interface NodeVersion {
  version: string
  date: string
  lts: boolean
  lts_codename: string | null
  files: string[]
  installed: boolean | null
  active: boolean | null
}

export interface ProjectInfo {
  name: string
  path: string
  version: string
  default_script?: string | null
  command_prefix?: string | null
}

export interface ProjectConfig {
  default_script: string | null
  command_prefix: string | null
}

export interface GitBranchInfo {
  name: string
  is_current: boolean
}

export interface GitBranches {
  branches: GitBranchInfo[]
}
