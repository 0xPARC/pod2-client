import type { Value } from "@pod2/pod2js";
import React from "react";
import ValueRenderer from "./ValueRenderer";

interface ValueTableProps {
  values: Value[];
}

export const ValueTable: React.FC<ValueTableProps> = ({ values }) => {
  return (
    <div className="border rounded-md">
      <table className="w-full">
        <thead>
          <tr className="border-b bg-gray-50 dark:bg-gray-800">
            <th
              className="px-2 py-1 text-left text-xs font-medium text-gray-700 dark:text-gray-300"
              title="Index"
            ></th>
            <th className="px-2 py-1 text-left text-xs font-medium text-gray-700 dark:text-gray-300">
              Value
            </th>
          </tr>
        </thead>
        <tbody>
          {values.map((value, index) => (
            <tr
              key={index}
              className={index < values.length - 1 ? "border-b" : ""}
            >
              <td className="px-2 py-1 text-sm text-gray-500 dark:text-gray-400">
                {index}
              </td>
              <td className="px-2 py-1 text-sm">
                <ValueRenderer value={value} />
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
};
