export interface SkillInfo {
  name: string;
  description: string;
  when: string | null;
  modes: string[];
  tags: string[];
  readOnly: boolean;
  source: string;
}
