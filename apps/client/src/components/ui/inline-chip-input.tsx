import { PlusIcon, XIcon } from "lucide-react";
import { useState, KeyboardEvent } from "react";
import { Button } from "./button";

interface InlineChipInputProps {
  label: string;
  placeholder: string;
  values: string[];
  onValuesChange: (values: string[]) => void;
  className?: string;
}

export function InlineChipInput({
  label,
  placeholder,
  values,
  onValuesChange,
  className
}: InlineChipInputProps) {
  const [inputValue, setInputValue] = useState("");

  const addValue = () => {
    const trimmedValue = inputValue.trim();
    if (trimmedValue && !values.includes(trimmedValue)) {
      onValuesChange([...values, trimmedValue]);
      setInputValue("");
    }
  };

  const removeValue = (valueToRemove: string) => {
    onValuesChange(values.filter((value) => value !== valueToRemove));
  };

  const handleKeyPress = (e: KeyboardEvent<HTMLInputElement>) => {
    if (e.key === "Enter") {
      e.preventDefault();
      addValue();
    }
  };

  const handleBlur = () => {
    // Add on blur if the input has content
    if (inputValue.trim()) {
      addValue();
    }
  };

  return (
    <div className={`flex items-center gap-2 ${className || ""}`}>
      <span className="text-sm text-muted-foreground">{label}:</span>
      <div className="flex items-center gap-1">
        {values.map((value) => (
          <div
            key={value}
            className="inline-flex items-center gap-1 px-2 py-1 bg-muted rounded-md text-sm"
          >
            <span>{value}</span>
            <button
              onClick={() => removeValue(value)}
              className="hover:text-destructive"
              type="button"
            >
              <XIcon className="h-3 w-3" />
            </button>
          </div>
        ))}
        <input
          type="text"
          placeholder={values.length === 0 ? placeholder : ""}
          value={inputValue}
          onChange={(e) => setInputValue(e.target.value)}
          onKeyPress={handleKeyPress}
          onBlur={handleBlur}
          autoComplete="off"
          autoCorrect="off"
          className="bg-transparent border-none outline-none text-sm placeholder:text-muted-foreground w-20 min-w-[5rem]"
        />
        <Button
          type="button"
          variant="ghost"
          size="sm"
          onClick={addValue}
          data-chip-add-button
        >
          <PlusIcon className="h-3 w-3" />
        </Button>
      </div>
    </div>
  );
}
