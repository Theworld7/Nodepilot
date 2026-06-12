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
}
