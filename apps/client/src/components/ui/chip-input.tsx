import { PlusIcon, XIcon } from "lucide-react";
import { useState, KeyboardEvent } from "react";
import { Badge } from "./badge";
import { Button } from "./button";
import { Input } from "./input";
import { Label } from "./label";

interface ChipInputProps {
  label: string;
  placeholder: string;
  values: string[];
  onValuesChange: (values: string[]) => void;
  variant?: "secondary" | "outline" | "default";
  helpText?: string;
  className?: string;
}

export function ChipInput({
  label,
  placeholder,
  values,
  onValuesChange,
  variant = "secondary",
  helpText,
  className
}: ChipInputProps) {
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
    <div className={`space-y-2 ${className || ""}`}>
      <Label>{label}</Label>
      <div className="flex gap-2">
        <Input
          placeholder={placeholder}
          value={inputValue}
          onChange={(e) => setInputValue(e.target.value)}
          onKeyPress={handleKeyPress}
          onBlur={handleBlur}
          autoComplete="off"
          className="flex-1"
        />
        <Button
          type="button"
          variant="outline"
          size="sm"
          onClick={addValue}
          data-chip-add-button
        >
          <PlusIcon className="h-4 w-4" />
        </Button>
      </div>
      {values.length > 0 && (
        <div className="flex flex-wrap gap-2 mt-2">
          {values.map((value) => (
            <Badge
              key={value}
              variant={variant}
              className="flex items-center gap-1"
            >
              {value}
              <button
                onClick={() => removeValue(value)}
                className="ml-1 hover:text-destructive"
                type="button"
              >
                <XIcon className="h-3 w-3" />
              </button>
            </Badge>
          ))}
        </div>
      )}
      {helpText && <p className="text-sm text-muted-foreground">{helpText}</p>}
    </div>
  );
}
