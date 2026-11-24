#pragma once

#include "CoreMinimal.h"
#include "Kismet/BlueprintFunctionLibrary.h"
#include "ZColorPickerBridgeLibrary.generated.h"

UCLASS()
class ZCOLORPICKERBRIDGE_API UZColorPickerBridgeLibrary : public UBlueprintFunctionLibrary
{
	GENERATED_BODY()

public:
	UFUNCTION(BlueprintCallable, Category = "Z Color Picker")
	static FString ApplyGradient(const FString& AssetPath, const TArray<FLinearColor>& Colors);
};
