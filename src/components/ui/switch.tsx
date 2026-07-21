type SwitchProps = {
  id?: string;
  checked: boolean;
  onCheckedChange: (checked: boolean) => void;
  "aria-label"?: string;
};

export function Switch({ id, checked, onCheckedChange, ...props }: SwitchProps) {
  return (
    <button
      id={id}
      type="button"
      role="switch"
      aria-checked={checked}
      className="ui-switch"
      data-checked={checked}
      onClick={() => onCheckedChange(!checked)}
      {...props}
    >
      <span className="ui-switch-thumb" aria-hidden="true" />
    </button>
  );
}
