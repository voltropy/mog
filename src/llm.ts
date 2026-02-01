const DEFAULT_MODEL_SIZE = "medium"

const DEFAULT_REASONING_EFFORT = "none"

type ModelSize = "small" | "medium" | "large"

type ReasoningEffort = "none" | "medium" | "high" | "xhigh"

type Table = Map<unknown, unknown>

type LLMOptions = {
  prompt: string
  model_size?: ModelSize
  reasoning_effort?: ReasoningEffort
  context?: string[]
}

function isString(value: unknown): value is string {
  return typeof value === "string"
}

function isError(value: unknown): value is Error {
  return value instanceof Error
}

export function llm(params: LLMOptions, returnType: string): unknown {
  if (!params.prompt || !isString(params.prompt)) {
    throw new Error("LLM operation requires 'prompt' as a string")
  }

  const modelSize = params.model_size ?? DEFAULT_MODEL_SIZE
  const reasoningEffort = params.reasoning_effort ?? DEFAULT_REASONING_EFFORT
  const context = params.context ?? []

  if (!["small", "medium", "large"].includes(modelSize)) {
    throw new Error(`Invalid model_size: ${modelSize}. Must be one of: small, medium, large`)
  }

  if (!["none", "medium", "high", "xhigh"].includes(reasoningEffort)) {
    throw new Error(`Invalid reasoning_effort: ${reasoningEffort}. Must be one of: none, medium, high, xhigh`)
  }

  if (!Array.isArray(context)) {
    throw new Error("LLM context must be an array of strings")
  }

  const systemPrompt = buildTypeSystemPrompt(returnType)

  return generateLLMOutput(params.prompt, systemPrompt, context, modelSize, reasoningEffort)
}

function buildTypeSystemPrompt(returnType: string): string {
  const typeInstructions: Record<string, string> = {
    string: "Respond with a single string without any additional text or formatting.",
    number: "Respond with a single number without any additional text or formatting.",
    boolean: "Respond with either 'true' or 'false' without any additional text.",
    i32: "Respond with a single 32-bit integer number without any additional text or formatting.",
    f32: "Respond with a single 32-bit floating point number without any additional text or formatting.",
    array: "Respond with a JSON array without any additional text or formatting.",
    table: "Respond with a JSON object (table) without any additional text or formatting.",
  }

  const instruction =
    typeInstructions[returnType] || `Generate a valid ${returnType} value without any additional text or formatting.`

  return `You are a type-safe output generator. ${instruction}\n\nOnly output the value itself, formatted correctly for the type.`
}

async function generateLLMOutput(
  prompt: string,
  systemPrompt: string,
  context: string[],
  modelSize: ModelSize,
  reasoningEffort: ReasoningEffort,
): Promise<unknown> {
  const contextStr = context.length > 0 ? `\n\nContext:\n${context.join("\n")}` : ""

  const fullPrompt = `${prompt}${contextStr}`

  try {
    const result = await performLLMCall(fullPrompt, systemPrompt, modelSize, reasoningEffort)

    return result
  } catch (error) {
    if (isError(error)) {
      throw new Error(`LLM call failed: ${error.message}`)
    }
    throw new Error("LLM call failed with unknown error")
  }
}

async function performLLMCall(
  prompt: string,
  systemPrompt: string,
  modelSize: ModelSize,
  reasoningEffort: ReasoningEffort,
): Promise<unknown> {
  const message = "LLM operation not yet implemented. Configure LLM provider to enable."

  console.warn(message)

  return prompt
}

export async function map_llm(collection: unknown[], params: LLMOptions, returnType: string): Promise<unknown[]> {
  const promises = collection.map((item) => {
    const itemContext = item ? String(item) : ""
    const context = [...(params.context || []), itemContext]
    const itemParams = { ...params, context }
    return llm(itemParams, returnType)
  })

  return Promise.all(promises)
}

export type { ModelSize, ReasoningEffort, LLMOptions }
